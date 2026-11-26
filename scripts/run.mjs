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
];
const root = fileURLToPath(new URL('../', import.meta.url));
const read = path => JSON.parse(readFileSync(path, 'utf8'));
const text = value => typeof value === 'string' && value.trim().length > 0;

function command(bin, args, capture = false) {
  const result = spawnSync(bin, args, { stdio: capture ? ['ignore', 'pipe', 'inherit'] : ['ignore', 'inherit', 'inherit'], encoding: 'utf8', timeout: 2 * 60 * 60 * 1000 });
  if (result.error) throw result.error;
  if (capture) process.stdout.write(result.stdout ?? '');
  if (result.status !== 0) throw Error(`${bin} stopped (${result.signal ?? result.status})`);
  return result.stdout;
}

export function verify({ target, buildBins, cases = [] }, exec = command) {
  exec('cargo', ['fmt', '--all', '--', '--check']);
  const output = exec('cargo', ['test', '--locked', '--workspace', '--test', target], true);
  if (!/test result: ok\. [1-9]\d* passed; 0 failed; 0 ignored/.test(output)) throw Error(`No passing, enabled ${target} suite`);
  const lines = output.split(/\r?\n/);
  for (const name of cases) {
    if (!lines.includes(`test ${name} ... ok`)) throw Error(`Missing passing ${target} case: ${name}`);
  }
  exec('cargo', ['test', '--locked', '--workspace']);
  if (buildBins) exec('cargo', ['build', '--locked', '--workspace', '--bins']);
}

export function validLimit(count) {
  return /^[1-9]\d*$/.test(count) && Number(count) <= packets.length;
}

export function runQueue(limit, work, check) {
  if (!validLimit(limit)) throw Error(`Expected 1-${packets.length} packets`);
  mkdirSync('.rustday', { recursive: true });
  mkdirSync('handoffs', { recursive: true });
  // Exclusive create: a second runner must never dispatch the same packet.
  writeFileSync('.rustday/lock', `${process.pid}\n`, { flag: 'wx' });
  try {
    for (const packet of packets) {
      const { id } = packet;
      if (existsSync(`.rustday/${id}.ok`)) continue;
      if (limit-- === 0) break;
      const path = `handoffs/${id}.json`;
      const prior = existsSync(path) ? readFileSync(path, 'utf8') : null;
      if (prior !== null) { writeFileSync(`.rustday/${id}.previous.json`, prior); rmSync(path); }
      work(packet);
      const handoff = read(path);
      if (handoff.status !== 'ready') throw Error(`${id}: ${handoff.summary || 'incomplete'}`);
      if (!text(handoff.summary) || !text(handoff.next) || !Array.isArray(handoff.evidence) || !handoff.evidence.length || !handoff.evidence.every(text)) throw Error(`Incomplete ${path}`);
      check(packet);
      writeFileSync(`.rustday/${id}.ok`, new Date().toISOString(), { flag: 'wx' });
      console.log(`Verified ${id}`);
    }
  } finally { rmSync('.rustday/lock'); }
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  process.chdir(root);
  const [mode = 'status', count, ...extra] = process.argv.slice(2);
  try {
    if (mode === 'status' && count === undefined) {
      for (const { id } of packets) console.log(id, existsSync(`.rustday/${id}.ok`) ? 'verified' : 'pending');
    } else if (mode === 'run' && validLimit(count) && !extra.length) {
      const cli = process.env.OPENCODE_BIN || 'opencode';
      command('cargo', ['--version']);
      command(cli, ['--version']);
      runQueue(Number(count), ({ id, database }) => {
        if (database && !process.env.RUSTDAY_TEST_DATABASE_URL) throw Error('Set RUSTDAY_TEST_DATABASE_URL to the disposable Rustday DB.');
        command(cli, ['run', '--standalone', '--agent', 'build', '--title', `Rustday ${id}`, `Complete only ${id} from ROADMAP.md. Read AGENTS.md, prior handoffs and any .rustday/${id}.previous.json. Inspect existing edits before continuing. Write handoffs/${id}.json and stop.`]);
      }, verify);
    } else throw Error(`Usage: node scripts/run.mjs status | run <1-${packets.length}>`);
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
