# boardcraft

boardcraft is an independent native workspace for organizing work on boards. It rebuilds, in Rust, the board workflows our team relies on — items, groups, sessions, permissions and failure recovery — without a third-party product in the loop.

Everything here is our own work: our schema, our rules, our controls, our names and our fixtures. This repository contains no other product's code, assets or wording, and its files must not name one. [AGENTS.md](AGENTS.md#independence) states the rule that applies to every change.

**First workflow:** Two items on a board are both called “Review”. Rename the second one to “Review artwork”, close the app, then reopen the board. The first row should still say “Review”; the second should keep its new name. The edit targets the item's ID, so matching titles never decide which row changes.

**Available today:** The [board integration test](crates/board/tests/board_edit.rs) verifies the rename in memory. The [PostgreSQL test](crates/board/tests/postgres_board.rs) checks persisted results after reconnecting. The desktop editor's [row-boundary test](crates/desktop/tests/desktop_edit.rs) covers save, failure and retry. The [Windows window test](crates/desktop/tests/windows_smoke.rs) verifies rejection, focus, correction and persistence across app restart. Follow the [development runbook](docs/development.md#windows-inline-edit-acceptance) to reproduce it. The [workflow register](docs/workflows.md) tracks what is verified and what is still open.

Windows and Android are the active targets; iOS follows Android, once Android's checks pass and a Mac is available. Rust owns the rules for board operations, PostgreSQL stores saved board data, and [Longbridge GPUI Kit](https://github.com/longbridge/gpui-kit) supplies the desktop controls.

## Open a board in the browser

From the checkout root on Windows:

```sh
cargo run --locked --manifest-path tools/Cargo.toml -- viewer
```

The Rust viewer builds the GPUI WebAssembly client and serves an editable synthetic board. Open the printed loopback URL in your normal browser, sign in with the disposable credentials you chose, and press Enter in the terminal to stop the host. See [viewer setup and rebuilds](docs/development.md#browser-viewer).

## Requirements

Install Rust with Cargo, rustfmt and clippy. Only worker dispatch needs OpenCode v2 with a configured provider. Desktop packets add Visual Studio's **Desktop development with C++** workload, the Windows SDK, CMake and a graphical session. PostgreSQL packets need PostgreSQL 17 and a disposable database of their own. Cargo build scripts and tests run with your account's permissions, so use a checkout and development account that hold no production credentials. A fresh public checkout must build without private material; the [development runbook](docs/development.md) names the tools and environment variables.

## Run the queue

```sh
cargo test --locked --manifest-path tools/Cargo.toml
cargo run --locked --manifest-path tools/Cargo.toml -- status
cargo run --locked --manifest-path tools/Cargo.toml -- postgres exec -- cargo run --locked --manifest-path tools/Cargo.toml -- run 3
```

`run 3` processes up to three pending packets; `run 1` is a single-milestone trial. Each gets a new `opencode run` session, with compaction available inside the session. The worker writes a short handoff; the runner checks it, runs the named Cargo suite and workspace checks, then records a verified checkpoint. The next session reads the handoff and the actual files, not a replay of the previous chat. The runner stops at the end of the queue, on a blocker, after a failed check, or on a command timeout (two hours by default). Packet 05 also needs the [dedicated x86_64 Android emulator and NDK linker](docs/development.md#android-emulator-edit-gate); its preflight and focused runtime gate have shorter deadlines.

Use Computer Use for live UI observations and GPUI Kit tests for control behavior. The former browser and Android UI drivers are historical. Host/store tests still check access, accepted edits and rollback; a packet missing an essential live observation stays pending.

The last command uses the Windows local database helper. With `RUSTDAY_TEST_DATABASE_URL` already configured for a disposable database, use `cargo run --locked --manifest-path tools/Cargo.toml -- run 3` directly. If the OpenCode CLI is not on PATH, set `OPENCODE_BIN` to its executable.

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

To extend the queue, register the packet and its inherited outcomes in [tools/src/packets.rs](tools/src/packets.rs) and update the [gate inventory](tools/tests/cutover-gates.tsv). Run the tools tests and check `status` before dispatch. A historical checkpoint does not verify a replacement client.
