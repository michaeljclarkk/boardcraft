# boardcraft

boardcraft is an independent native workspace for organizing work on boards. It rebuilds, in Rust, the board workflows our team relies on — items, groups, sessions, permissions and failure recovery — without a third-party product in the loop.

Everything here is our own work: our schema, our rules, our controls, our names and our fixtures. This repository contains no other product's code, assets or wording, and its files must not name one. [AGENTS.md](AGENTS.md#independence) states the rule that applies to every change.

**First workflow:** Two items on a board are both called “Review”. Rename the second one to “Review artwork”, close the app, then reopen the board. The first row should still say “Review”; the second should keep its new name. The edit targets the item's ID, so matching titles never decide which row changes.

**Available today:** The [board integration test](crates/board/tests/board_edit.rs) verifies the rename in memory. The [PostgreSQL test](crates/board/tests/postgres_board.rs) checks persisted results after reconnecting. The desktop editor's [row-boundary test](crates/desktop/tests/desktop_edit.rs) covers save, failure and retry. The [Windows window test](crates/desktop/tests/windows_smoke.rs) verifies rejection, focus, correction and persistence across app restart. Follow the [development runbook](docs/development.md#windows-inline-edit-acceptance) to reproduce it. The [workflow register](docs/workflows.md) tracks what is verified and what is still open.

Windows and Android are the active targets; iOS follows Android, once Android's checks pass and a Mac is available. Rust owns the rules for board operations, PostgreSQL stores saved board data, and [Longbridge GPUI Kit](https://github.com/longbridge/gpui-kit) supplies the desktop controls.

## Requirements

Node 22+, OpenCode v2 with a configured provider, and a Rust toolchain with Cargo, rustfmt and clippy. Desktop packets add Visual Studio's **Desktop development with C++** workload, the Windows SDK, CMake and a graphical session. PostgreSQL packets need PostgreSQL 17 and a disposable database of their own. Cargo build scripts and tests run with your account's permissions, so use a checkout and development account that hold no production credentials. A fresh public checkout must build without private material; the [development runbook](docs/development.md) names the tools and environment variables.

## Run the queue

```sh
node --test scripts/run.test.mjs
node scripts/run.mjs status
node scripts/run.mjs run 3
```

`run 3` processes up to three pending packets; `run 1` is a single-milestone trial. Each gets a new `opencode run` session, with compaction available inside the session. The worker writes a short handoff; the runner checks it, runs the named Cargo suite and workspace checks, then records a verified checkpoint. The next session reads the handoff and the actual files, not a replay of the previous chat. The runner stops at the end of the queue, on a blocker, after a failed check, or on a command timeout (two hours by default). Packet 05 also needs the [dedicated x86_64 Android emulator and NDK linker](docs/development.md#android-emulator-edit-gate); its preflight and focused runtime gate have shorter deadlines.

Packets 06 and 07 use the [Kit APK setup](docs/development.md#android-kit-frame-gate) on that same dedicated emulator with host GPU. [07](packets/07-android-kit-edit.md) also needs the disposable PostgreSQL cluster. Its gate installs the current APK, types into the Kit editor, checks visible failure and recovery, and reconnects to verify the saved rows.

If the CLI is not on PATH, set `OPENCODE_BIN` to its executable. On Windows with OpenCode Desktop already running, PowerShell can use `$env:OPENCODE_BIN = (Get-Process opencode-cli | Select-Object -First 1 -ExpandProperty Path)`.

**Resume:** inspect the last handoff and terminal error, fix the blocker, and run the same command in the same checkout. Partial edits remain on disk. New checkouts keep runner state in `.boardcraft/`; existing `.rustday/` state is reused in place, including locks, verified packets and prior handoffs. If both directories exist, the runner stops. After an abrupt shutdown, confirm that the runner and worker have exited before removing a stale `lock` from the active state directory. Verification markers are local to this checkout; invalidate the affected marker and later markers if you revert completed work. Review and commit source and public-safe handoffs at milestones; the runner does not publish changes.

## Where to read next

| Question | Read |
|---|---|
| What do I build next, and what counts as done? | [ROADMAP.md](ROADMAP.md) plus your own file in [packets](packets/) |
| How do I build, run and recover the environment? | [docs/development.md](docs/development.md) |
| Where does behaviour live? | [docs/architecture.md](docs/architecture.md) |
| What is verified, and what is still unknown? | [docs/workflows.md](docs/workflows.md) |
| Which document do I update for this change? | [docs/README.md](docs/README.md) |
| How must a worker behave? | [AGENTS.md](AGENTS.md) |

To extend the queue, append a packet in `scripts/run.mjs` with its ID, named test target, database requirement and any additional required cases or build. The maximum `run` count follows the queue length. Run the Node tests and check `status` before dispatching a worker.
