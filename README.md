# boardcraft

boardcraft is a native workspace for organizing work on boards. boardcraft is inspired by monday dot com, but does not, and will not ever use any of it's code, branding or assets.

**First workflow:** Two items on a board are both called “Review”. Rename the second one to “Review artwork”, close the app, then reopen the board. The first row should still say “Review”; the second should keep its new name. The edit targets the item's ID, so matching titles never decide which row changes.

**Available today:** The [board integration test](crates/board/tests/board_edit.rs) verifies the rename in memory. The [PostgreSQL test](crates/board/tests/postgres_board.rs) checks persisted results after reconnecting. The desktop editor's [row-boundary test](crates/desktop/tests/desktop_edit.rs) covers save, failure and retry. The [Windows window test](crates/desktop/tests/windows_smoke.rs) verifies rejection, focus, correction and persistence across app restart. Follow the [development runbook](docs/development.md#windows-inline-edit-acceptance) to reproduce it. The [workflow register](docs/workflows.md) tracks remaining parity gaps; the [documentation map](docs/README.md) links the plan, owners and evidence.

Windows and Android are the active targets; iOS follows Android. Rust owns the rules for board operations, PostgreSQL stores saved board data, and [Longbridge GPUI Kit](https://github.com/longbridge/gpui-kit) supplies the desktop controls.

## Run the migration

Use Node 22+, OpenCode v2 with a configured provider, and a Rust toolchain with Cargo and rustfmt. For PostgreSQL, desktop and mobile service packets, supply `RUSTDAY_TEST_DATABASE_URL` for a disposable boardcraft PostgreSQL database. The desktop packet also needs a graphical session and the platform's native GPUI build prerequisites. Use a dedicated checkout and development account with no production credentials. Cargo build scripts and tests run with the account's permissions.

```sh
node --test scripts/run.test.mjs
node scripts/run.mjs status
node scripts/run.mjs run 3
```

`run 3` processes up to three pending packets; `run 1` is a single-milestone trial. Each gets a new `opencode run` session, with compaction available inside the session. The worker writes a short handoff; the runner checks it, runs the named Cargo suite and workspace checks, then records a verified checkpoint. The next session reads the handoff and the actual files, not a replay of the previous chat. The runner stops at the end of the queue, on a blocker, after a failed check, or on a command timeout (two hours per invocation).

If the CLI is not on PATH, set `OPENCODE_BIN` to its executable. On Windows with OpenCode Desktop already running, PowerShell can use `$env:OPENCODE_BIN = (Get-Process opencode-cli | Select-Object -First 1 -ExpandProperty Path)`.

**Resume:** inspect the last handoff and terminal error, fix the blocker, and run the same command in the same checkout. Partial edits remain on disk. New checkouts keep runner state in `.boardcraft/`; existing `.rustday/` state is reused in place, including locks, verified packets and prior handoffs. If both directories exist, the runner stops. After an abrupt shutdown, confirm that the runner and worker have exited before removing a stale `lock` from the active state directory. Verification markers are local to this checkout; invalidate the affected marker and later markers if you revert completed work. Review and commit source and public-safe handoffs at milestones; the runner does not publish changes.

See [ROADMAP.md](ROADMAP.md) for acceptance tests and [AGENTS.md](AGENTS.md) for the worker contract. To extend the queue, append a packet in `scripts/run.mjs` with its ID, named test target, database requirement and any additional required cases/build. The maximum `run` count follows the queue length. Run the Node tests and check `status` before dispatching a worker.
