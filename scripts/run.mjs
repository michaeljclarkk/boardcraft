import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { resolve } from 'node:path';

export const packets = [ ['01-board', 'board_edit'], ['02-postgres', 'postgres_board'], ['03-desktop', 'desktop_edit'] ];
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

function verify(id, target) {
  command('cargo', ['fmt', '--all', '--', '--check']);
  const output = command('cargo', ['test', '--locked', '--workspace', '--test', target], true);
  if (!/test result: ok\. [1-9]\d* passed; 0 failed; 0 ignored/.test(output)) throw Error(`No passing, enabled ${target} suite`);
  command('cargo', ['test', '--locked', '--workspace']);
  if (id === '03-desktop') command('cargo', ['build', '--locked', '--workspace', '--bins']);
}

export function runQueue(limit, work, check) {
  mkdirSync('.rustday', { recursive: true });
  mkdirSync('handoffs', { recursive: true });
  // Exclusive create: a second runner must never dispatch the same packet.
  writeFileSync('.rustday/lock', `${process.pid}\n`, { flag: 'wx' });
  try {
    for (const [id, target] of packets) {
      if (existsSync(`.rustday/${id}.ok`)) continue;
      if (limit-- === 0) break;
      const path = `handoffs/${id}.json`;
      const prior = existsSync(path) ? readFileSync(path, 'utf8') : null;
      if (prior !== null) { writeFileSync(`.rustday/${id}.previous.json`, prior); rmSync(path); }
      work(id);
      const handoff = read(path);
      if (handoff.status !== 'ready') throw Error(`${id}: ${handoff.summary || 'incomplete'}`);
      if (!text(handoff.summary) || !text(handoff.next) || !Array.isArray(handoff.evidence) || !handoff.evidence.length || !handoff.evidence.every(text)) throw Error(`Incomplete ${path}`);
      check(id, target);
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
      for (const [id] of packets) console.log(id, existsSync(`.rustday/${id}.ok`) ? 'verified' : 'pending');
    } else if (mode === 'run' && /^[1-3]$/.test(count) && !extra.length) {
      const cli = process.env.OPENCODE_BIN || 'opencode';
      command('cargo', ['--version']);
      command(cli, ['--version']);
      runQueue(Number(count), id => {
        if (id !== '01-board' && !process.env.RUSTDAY_TEST_DATABASE_URL) throw Error('Set RUSTDAY_TEST_DATABASE_URL to the disposable Rustday DB.');
        command(cli, ['run', '--standalone', '--agent', 'build', '--title', `Rustday ${id}`, `Complete only ${id} from ROADMAP.md. Read AGENTS.md, prior handoffs and any .rustday/${id}.previous.json. Inspect existing edits before continuing. Write handoffs/${id}.json and stop.`]);
      }, verify);
    } else throw Error('Usage: node scripts/run.mjs status | run <1-3>');
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
