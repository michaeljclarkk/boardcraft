// Drive a freshly installed, foreground Kit app. Only the host fixture can read PostgreSQL.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import { run, withKitApp } from './android-kit-frame.mjs';
import { requireKitEmulator } from './run.mjs';

export function editFrame(lines, expected, absent = []) {
  if (!Array.isArray(lines) || lines.some(l => absent.includes(l.text))) return;
  let y = -1;
  const hits = {};
  for (const text of expected) {
    const found = lines.filter(l => l.text === text);
    if (found.length !== 1 || !Number.isSafeInteger(found[0].x) || found[0].x <= 0 ||
        !Number.isSafeInteger(found[0].y) || found[0].y <= y) return;
    hits[text] = found[0];
    y = found[0].y;
  }
  return hits;
}

async function bounded(promise, ms, what) {
  let timer;
  try {
    return await Promise.race([promise, new Promise((_, reject) => {
      timer = setTimeout(() => reject(Error(`Timed out waiting for ${what}`)), ms);
    })]);
  } finally { clearTimeout(timer); }
}

async function withFixture(journey) {
  const name = 'kit_edit_fixture';
  run('cargo', ['build', '--locked', '-p', 'boardcraft-mobile-edit-host', '--example', name], {inherit: true, timeout: 600_000});
  const meta = JSON.parse(run('cargo', ['metadata', '--locked', '--no-deps', '--format-version', '1']));
  const child = spawn(join(meta.target_directory, 'debug', 'examples', `${name}.exe`), [], {stdio:['pipe','pipe','inherit']});
  const exit = new Promise((res, rej) => { child.once('exit', res); child.once('error', rej); });
  const lines = createInterface({input: child.stdout, crlfDelay:Infinity})[Symbol.asyncIterator]();
  const line = async () => {
    const r = await bounded(lines.next(), 30_000, 'fixture response');
    if (r.done) throw Error('Fixture exited without response');
    return r.value;
  };
  try {
    const {board, port, schema} = JSON.parse(await line());
    if (!/^[a-f\d]{8}(?:-[a-f\d]{4}){3}-[a-f\d]{12}$/i.test(board) || !Number.isInteger(port) || port < 1 || port > 65535) throw Error('Invalid loopback fixture');
    await journey(board, port, async state => {
      child.stdin.write(`${state}\n`);
      if (await line() !== 'ok') throw Error(`Store mismatch: ${state}`);
      console.log(`Fresh store connection: ${state}`);
    }, schema);
  } finally {
    child.stdin.end();
    try { if (await bounded(exit, 30_000, 'fixture cleanup') !== 0) throw Error('Fixture failed'); }
    finally { if (child.exitCode === null && child.signalCode === null) child.kill(); }
  }
}

export async function gate(afterSave) {
  requireKitEmulator();
  await withKitApp(async ({adb, start, frame, tap, foreground}) => withFixture(async (board, port, check, schema) => {
    // A concrete ephemeral host port avoids the Windows adb tcp:0 cache issue (05).
    const spec = `tcp:${port}`;
    adb('reverse', '--no-rebind', spec, spec);
    try {
      start('--es', 'board', board, '--ei', 'port', String(port));
      const boardText = ['Sample board', 'Review sketch'];
      const screen = async (name, texts, absent = [], required = []) => {
        const hits = await frame(name, l => required.every(t => l.some(row => row.text === t)) && editFrame(l, [...boardText, ...texts], absent));
        console.log(`${name}: ${Object.keys(hits).concat(required).join(' / ')}`);
        return hits;
      };
      const input = (...args) => { if (!foreground()) throw Error('Fixture left foreground'); adb('shell','input',...args); };
      const ime = async shown => {
        const deadline = Date.now() + 8000;
        do {
          if (foreground() && new RegExp(`\\bmInputShown=${shown}\\b`).test(adb('shell','dumpsys','input_method'))) return;
          await delay(250);
        } while (Date.now() < deadline);
        throw Error(`Expected IME shown=${shown}`);
      };
      tap((await screen('loaded', ['Ink'], ['Editing name','Save'])).Ink);
      const editor = await screen('opened', ['Editing name','Ink','Save']);
      // Tap just to the right of this short value, inside the full-width Kit Input.
      const end = {...editor.Ink, x: editor.Ink.x + 48};
      tap(end);
      await ime(true);
      input('keyevent','67','67','67');
      const blank = await screen('cleared', ['Editing name','Save'], ['Ink','Coral']);
      tap(blank.Save);
      const rejected = await screen('rejected', ['Editing name','Save'], ['Ink','Coral'], ['Name required']);
      await check('original');
      tap({...end, y: rejected['Editing name'].y + end.y - editor['Editing name'].y});
      await ime(true);
      input('text','Coral');
      const draft = ['Editing name','Coral','Save'];
      await screen('corrected draft', draft, ['Ink']);
      input('keyevent','4');
      await ime(false);
      const dismissed = await screen('dismissed keyboard', draft, ['Ink']);
      await check('original');
      tap(dismissed.Save);
      await screen('committed', ['Coral'], ['Ink','Editing name','Save','Name required']);
      await check('saved');
      if (afterSave) await afterSave({board, port, schema, check, adb, start, screen, tap, input, ime, foreground});
    } finally { requireKitEmulator(); adb('reverse','--remove', spec); }
  }));
  console.log('Android Kit: cleared, rejected, corrected, dismissed IME with Back, saved; store readbacks agree');
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  process.chdir(fileURLToPath(new URL('../', import.meta.url)));
  await gate();
}
