// Continue the actual 07 save, keeping its host, board and mapping alive.
import assert from 'node:assert/strict';
import { createServer } from 'node:net';
import { join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import { editFrame, gate } from './android-kit-edit.mjs';
import { run } from './android-kit-frame.mjs';
import { requireKitEmulator } from './run.mjs';

const pkg = 'dev.boardcraft.kitfixture';

// The server-side activity record alone can survive recreation of the Java activity.
export function instance(dump) {
  const activities = [...dump.matchAll(/^\s*ACTIVITY dev\.boardcraft\.kitfixture\/\S+ ([\da-f]+) pid=(\d+)\b/gm)];
  const local = [...dump.matchAll(/^\s*Local Activity ([\da-f]+) State:\s+mResumed=(true|false) mStopped=(true|false) mFinished=false\b/gm)];
  if (activities.length !== 1 || local.length !== 1) return;
  return {record: activities[0][1], pid: activities[0][2], local: local[0][1], resumed: local[0][2] === 'true', stopped: local[0][3] === 'true'};
}

async function waitFor(label, read) {
  const deadline = Date.now() + 15_000;
  do {
    const value = read();
    if (value) return value;
    await delay(250);
  } while (Date.now() < deadline);
  throw Error(`Did not observe ${label}`);
}

export function checkWindow(output, board) {
  const rows = output.split(/\r?\n/).filter(l => l.startsWith('row '));
  assert.deepEqual(rows, [
    `row ${board} 00000000-0000-0000-0000-00000000002a Review sketch`,
    `row ${board} 00000000-0000-0000-0000-000000000011 Coral`,
  ]);
  console.log(rows.join('\n'));
}

function desktop(board, schema) {
  if (!/^mobile_test_[a-f0-9]{32}$/.test(schema)) throw Error('Missing fixture schema');
  const url = new URL(process.env.RUSTDAY_TEST_DATABASE_URL);
  if (url.pathname !== '/rustday_test' || !['127.0.0.1','localhost','[::1]'].includes(url.hostname)) throw Error('Expected disposable loopback DB');
  url.searchParams.set('options', `-csearch_path=${schema}`);
  const {target_directory} = JSON.parse(run('cargo', ['metadata','--no-deps','--locked','--format-version','1']));
  const output = run(join(target_directory,'debug','boardcraft-desktop.exe'), [board,'--smoke-delivery'], {
    timeout: 45_000, env: {...process.env, RUSTDAY_TEST_DATABASE_URL: url.href},
  });
  checkWindow(output, board);
}

async function journey({board, port, schema, check, adb, start, screen, tap, input, ime, foreground, signIn}) {
  const display = label => screen(label, ['Coral'], ['Ink','Amber','Editing name','Save']);
  const draft = label => screen(label, ['Editing name','Amber','Save'], ['Ink','Coral']);
  const activity = () => instance(adb('shell','dumpsys','activity',pkg));
  tap((await display('accepted before interruption')).Coral);
  const editor = await screen('reopened editor', ['Editing name','Coral','Save']);
  tap({...editor.Coral, x: editor.Coral.x + 64});
  await ime(true);
  input('keyevent','67','67','67','67','67');
  await screen('cleared second draft', ['Editing name','Save'], ['Coral','Ink','Amber']);
  input('text','Amber');
  await draft('unsaved draft');
  input('keyevent','4');
  await ime(false);
  await draft('draft before HOME');
  const before = activity();
  assert.ok(before?.resumed && !before.stopped, 'expected live foreground activity');
  input('keyevent','KEYCODE_HOME');
  await waitFor('stopped background activity', () => !foreground() && activity()?.stopped);
  start('--activity-reorder-to-front');
  await draft('resumed draft');
  assert.deepEqual(activity(), before, 'resume must return to the same local activity and PID');
  await check('saved');
  desktop(board, schema);
  requireKitEmulator();
  adb('shell','am','force-stop',pkg);
  await waitFor('fixture process exit', () => {
    const lines = adb('shell','ps','-A','-o','PID,NAME').trim().split(/\n/).map(l => l.trim().split(/\s+/));
    return lines[0]?.join(' ') === 'PID NAME' && lines.some(r => r[0] === '1') && !lines.some(r => r[1] === pkg);
  });
  // The cold reauthentication branch models an explicitly revoked session.
  await check('remember'); await check('revoke-original');
  start('--es','board',board,'--ei','port',String(port));
  await signIn(false);
  await display('cold reopened accepted board');
  const after = activity();
  assert.ok(after?.resumed && !after.stopped && after.pid !== before.pid, 'expected recreated activity in new process');
  await check('saved');
  console.log(`Board ${board}: HOME/resume kept PID ${before.pid} and local activity ${before.local}; cold reopen in PID ${after.pid}`);
}

async function restore({board, port, adb, start, frame, tap, check, signIn}) {
  const activity = () => instance(adb('shell','dumpsys','activity',pkg));
  const cold = async target => {
    requireKitEmulator();
    const before = activity(); assert.ok(before?.resumed && !before.stopped);
    adb('shell','am','force-stop',pkg);
    await waitFor('process exit', () => {
      const rows = adb('shell','ps','-A','-o','PID,NAME').trim().split(/\r?\n/).map(s=>s.trim().split(/\s+/));
      return rows[0]?.join(' ')==='PID NAME' && rows.some(r=>r[0]==='1') && !rows.some(r=>r[1]===pkg);
    });
    start('--es','board',board,'--ei','port',String(target));
    await waitFor('fresh foreground process', () => { const a=activity(); return a?.resumed && !a.stopped && a.pid!==before.pid; });
  };
  const noBoard = ['Sample board','Review sketch','Ink','Coral','Editing name','Amber','Save','Log out'];
  const prompt = () => frame('cold identifier', l => editFrame(l,['Username or email','Next'],[...noBoard,'Password','Retry connection']));
  const display = () => frame('accepted protected board', l => l.some(x=>x.text==='Log out') && editFrame(l,['Sample board','Review sketch','Coral'],['Editing name','Save','Ink','Amber','Retry connection','Next','Password']));
  const mapping = `tcp:${port}`;
  const disconnected = async work => {
    requireKitEmulator(); adb('reverse','--remove',mapping);
    try { await work(); } finally { requireKitEmulator(); adb('reverse','--no-rebind',mapping,mapping); }
  };

  await check('remember');
  await cold(port); await display(); await check('same-session');
  let connections = 0;
  const other = createServer(socket => { ++connections; socket.destroy(); });
  await new Promise((yes,no)=>{ other.once('error',no); other.listen(0,'127.0.0.1',yes); });
  try {
    const b = other.address().port; assert.notEqual(port,b);
    const spec = `tcp:${b}`; requireKitEmulator(); adb('reverse','--no-rebind',spec,spec);
    try {
      await cold(b); await prompt(); await check('same-session');
      await cold(port); await display(); await check('same-session');
      assert.equal(connections,0,'other origin received traffic');
    } finally { requireKitEmulator(); adb('reverse','--remove',spec); }
  } finally { await new Promise(resolve => other.close(resolve)); }

  const action = await frame('logout before disconnect', l=>l.find(x=>x.text==='Log out'));
  await disconnected(async () => {
    tap(action);
    await frame('retryable logout failure',l=>l.some(x=>x.text==='Could not log out') && editFrame(l,['Sample board','Review sketch','Coral'],['Next']));
    await check('same-session');
    await cold(port);
    await frame('offline resume',l=>editFrame(l,['Retry connection'],[...noBoard,'Next','Username or email','Password']));
    await check('same-session');
  });
  tap((await frame('explicit retry', l=>editFrame(l,['Retry connection'],noBoard)))['Retry connection']);
  await display(); await check('same-session');
  await check('revoke-original');
  await cold(port); await prompt(); await check('no-session');
  await disconnected(async()=>{ await cold(port); await prompt(); await check('no-session'); });
  await signIn(false); await display(); await check('remember');
  tap(await frame('explicit logout',l=>l.find(x=>x.text==='Log out')));
  await prompt(); await check('no-session');
  await disconnected(async()=>{ await cold(port); await prompt(); await check('no-session'); });
  console.log('Verified cold session restoration, origin isolation, retry, rejection and logout cleanup');
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  process.chdir(fileURLToPath(new URL('../', import.meta.url)));
  const args = process.argv.slice(2);
  if (args.length && (args.length !== 1 || args[0] !== '--restore')) throw Error('Expected [--restore]');
  let db;
  try { db = new URL(process.env.RUSTDAY_TEST_DATABASE_URL); }
  catch { throw Error('Expected disposable loopback database'); }
  if (!['postgres:', 'postgresql:'].includes(db.protocol) || db.pathname !== '/rustday_test' || !['127.0.0.1','localhost','[::1]'].includes(db.hostname)) throw Error('Expected disposable loopback database');
  requireKitEmulator();
  if (!args.length) run('cargo', ['build','--locked','-p','boardcraft-desktop','--features','native-smoke'], {inherit:true,timeout:3600_000});
  await gate(args.length ? restore : journey);
}
