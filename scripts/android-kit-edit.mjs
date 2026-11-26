// Drive a freshly installed, foreground Kit app. Only the host fixture can read PostgreSQL.
import { spawn, spawnSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
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

async function withFixture(journey, browse) {
  const name = 'kit_edit_fixture';
  run('cargo', ['build', '--locked', '-p', 'boardcraft-mobile-edit-host', '--example', name], {inherit: true, timeout: 600_000});
  const meta = JSON.parse(run('cargo', ['metadata', '--locked', '--no-deps', '--format-version', '1']));
  const child = spawn(join(meta.target_directory, 'debug', 'examples', `${name}.exe`), browse ? ['--browse'] : [], {stdio:['pipe','pipe','inherit']});
  const exit = new Promise((res, rej) => { child.once('exit', res); child.once('error', rej); });
  const lines = createInterface({input: child.stdout, crlfDelay:Infinity})[Symbol.asyncIterator]();
  const line = async () => {
    const r = await bounded(lines.next(), 30_000, 'fixture response');
    if (r.done) throw Error('Fixture exited without response');
    return r.value;
  };
  try {
    const password = randomBytes(16).toString('hex');
    child.stdin.write(`${password}\n`);
    const {board, port, schema} = JSON.parse(await line());
    if (!/^[a-f\d]{8}(?:-[a-f\d]{4}){3}-[a-f\d]{12}$/i.test(board) || !Number.isInteger(port) || port < 1 || port > 65535) throw Error('Invalid loopback fixture');
    await journey(board, port, async state => {
      child.stdin.write(`${state}\n`);
      if (await line() !== 'ok') throw Error(`Store mismatch: ${state}`);
      console.log(`Fresh store connection: ${state}`);
    }, schema, password);
  } finally {
    child.stdin.end();
    try { if (await bounded(exit, 30_000, 'fixture cleanup') !== 0) throw Error('Fixture failed'); }
    finally { if (child.exitCode === null && child.signalCode === null) child.kill(); }
  }
}

export async function gate(afterSave, browse = false) {
  requireKitEmulator();
  await withKitApp(async ({adb, start, frame, tap, foreground}) => withFixture(async (board, port, check, schema, password) => {
    // A concrete ephemeral host port avoids the Windows adb tcp:0 cache issue (05).
    const spec = `tcp:${port}`;
    adb('reverse', '--no-rebind', spec, spec);
    try {
      if (browse) start('-a', 'dev.boardcraft.BROWSE', '--ei', 'port', String(port));
      else start('--es', 'board', board, '--ei', 'port', String(port));
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
      // Send synthetic hardware key presses through a non-PTY pipe. Neither the
      // host adb argv nor an Android command line contains the password string.
      const typeMasked = value => {
        if (!foreground() || !/^[0-9a-f]+$/.test(value)) throw Error('Invalid fixture input');
        const commands = [...value].map(ch => `input keyevent ${ch <= '9' ? 7 + Number(ch) : 29 + ch.charCodeAt(0) - 97} || exit 1`);
        const r = spawnSync('adb', ['-s',process.env.RUSTDAY_ANDROID_SERIAL,'shell','-T','sh'], {
          input: commands.join('\n') + '\n', timeout: 45_000, stdio:['pipe','ignore','ignore'],
        });
        if (r.error || r.status !== 0) throw Error('Masked input failed');
      };
      const loginFrame = (label, texts) => frame(label, lines => {
        const hits = {};
        for (const text of texts) {
          const found = lines.filter(l => l.text === text);
          if (found.length !== 1) return;
          hits[text] = found[0];
        }
        return hits;
      });
      const hideIme = async () => { input('keyevent','4'); await ime(false); };
      const signIn = async rejectFirst => {
        const id = await loginFrame('identifier', ['Username or email','Next']);
        tap(id['Username or email']); await ime(true); input('text','boardmember'); await hideIme();
        tap((await loginFrame('continue', ['Next'])).Next);
        const form = await loginFrame('password', ['Password','Sign in']);
        tap(form.Password); await ime(true);
        if (rejectFirst) {
          typeMasked('0'); await hideIme(); tap(form['Sign in']);
          await loginFrame('rejected login', ['Incorrect username or password']);
          await check('rejected');
          tap(form.Password); await ime(true);
          input('keyevent','KEYCODE_MOVE_END'); input('keyevent','67');
        }
        typeMasked(password); await hideIme(); tap(form['Sign in']);
      };
      if (browse) {
        await signIn(false);
        return await browseJourney({check, frame, tap, input});
      }
      await signIn(true);
      const loaded = await screen('loaded', ['Ink'], ['Editing name','Save']);
      await check('signed-in');
      tap(loaded.Ink);
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
      if (afterSave) await afterSave({board, port, schema, check, adb, start, screen, tap, input, ime, foreground, frame, signIn});
    } finally { requireKitEmulator(); adb('reverse','--remove', spec); }
  }, browse));
  console.log(browse ? 'Android Kit: selected both boards, denied stale tap, restored membership and reopened by ID' : 'Android Kit: cleared, rejected, corrected, dismissed IME with Back, saved; store readbacks agree');
}

async function browseJourney({check, frame, tap, input}) {
  // Compare every fixture label, including duplicates. Ignore only OS chrome and
  // incidental OCR; a stale sibling, hidden board or editor must fail the match.
  const list = ['Boards','Quartz','Review','Review','Refresh','Log out'];
  const items = ['Board items','Sketch','Ink','Back to boards','Log out'];
  const sibling = ['Board items','Outline','Back to boards','Log out'];
  const denied = ['Destination unavailable','Back to boards','Log out'];
  const empty = ['No boards','Refresh','Log out'];
  const known = new Set([...list,...items,...sibling,...denied,...empty,'Sample board','Review sketch','Hidden','Save','Editing name','Username or email','Next','Empty board','Retry board list']);
  const screen = async (name, expected) => {
    const matched = await frame(name, lines => {
      const top = lines.find(r => r.text === expected[0]);
      const bottom = lines.find(r => r.text === 'Log out');
      if (!top || !bottom) return;
      const rows = lines.filter(r => known.has(r.text) || (r.y >= top.y && r.y <= bottom.y)).sort((a,b) => a.y-b.y);
      if (rows.length === expected.length && rows.every((r,i) => r.text === expected[i] && Number.isSafeInteger(r.x) && r.x > 0 && Number.isSafeInteger(r.y) && r.y > (rows[i-1]?.y ?? 0))) return rows;
    });
    console.log(`${name}: ${matched.map(r => r.text).join(' / ')}`);
    return matched;
  };
  const listed = await screen('initial list',list);
  await check('remember'); tap(listed[3]);
  await screen('second Review',items); await check('joined');
  input('keyevent','4');
  tap((await screen('system Back to list',list))[2]);
  const first = await screen('first Review',sibling); await check('joined'); tap(first[2]);
  const stale = await screen('list before revoke',list);
  await check('revoke-membership'); tap(stale[3]);
  await screen('unavailable',denied); await check('outside');
  input('keyevent','4');
  const cleared = await screen('empty list',empty); await check('outside');
  await check('restore-membership'); tap(cleared[1]);
  const restored = await screen('refreshed list',list); await check('joined'); tap(restored[3]);
  await screen('second Review after recovery',items); await check('joined');
}

async function logoutJourney({port, check, adb, screen, tap, input, ime, frame, signIn}) {
  const find = text => frame(text, lines => lines.find(l => l.text === text));
  await check('peer');
  const action = await find('Log out');
  const mapping = `tcp:${port}`;
  requireKitEmulator();
  adb('reverse', '--remove', mapping);
  try {
    tap(action);
    await find('Could not log out');
    await screen('board after failed logout', ['Coral'], ['Editing name','Next']);
    await check('logout-failed');
  } finally { requireKitEmulator(); adb('reverse','--no-rebind',mapping,mapping); }

  const replace = async (from, to) => {
    tap((await screen('board before edit', [from], ['Editing name']))[from]);
    const editor = await screen('edit before logout', ['Editing name',from,'Save']);
    tap({...editor[from], x: editor[from].x + 64});
    await ime(true);
    input('keyevent', ...Array(from.length).fill('67'));
    input('text', to);
    input('keyevent', '4'); await ime(false);
    return screen('draft before logout', ['Editing name',to,'Save']);
  };
  tap((await replace('Coral','Jade')).Save);
  await screen('saved with original session', ['Jade'], ['Save','Editing name']);
  await check('still-signed-in');
  await replace('Jade','Amber');
  tap(await find('Log out'));
  await frame('signed out', lines => editFrame(lines, ['Username or email','Next'], ['Sample board','Review sketch','Jade','Amber','Editing name','Save','Log out']));
  await check('logged-out');
  await signIn(false);
  await screen('signed in after logout', ['Jade'], ['Amber','Editing name','Save']);
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  process.chdir(fileURLToPath(new URL('../', import.meta.url)));
  const args = process.argv.slice(2);
  if (args.length && (args.length !== 1 || !['--logout','--browse'].includes(args[0]))) throw Error('Usage: android-kit-edit.mjs [--logout|--browse]');
  await gate(args[0] === '--logout' ? logoutJourney : undefined, args[0] === '--browse');
}
