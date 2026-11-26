// Continue the actual 07 save, keeping its host, board and mapping alive.
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import { gate } from './android-kit-edit.mjs';
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

async function journey({board, port, schema, check, adb, start, screen, tap, input, ime, foreground}) {
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
  start('--es','board',board,'--ei','port',String(port));
  await display('cold reopened accepted board');
  const after = activity();
  assert.ok(after?.resumed && !after.stopped && after.pid !== before.pid, 'expected recreated activity in new process');
  await check('saved');
  console.log(`Board ${board}: HOME/resume kept PID ${before.pid} and local activity ${before.local}; cold reopen in PID ${after.pid}`);
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  process.chdir(fileURLToPath(new URL('../', import.meta.url)));
  requireKitEmulator();
  run('cargo', ['build','--locked','-p','boardcraft-desktop','--features','native-smoke'], {inherit:true,timeout:3600_000});
  await gate(journey);
}
