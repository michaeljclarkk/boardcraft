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

The Rust viewer builds the GPUI WebAssembly client and opens **Review** in your default browser as the local owner. No username or password is needed. The terminal also prints the board address. Press Enter there to stop the host and discard the fixture. See [viewer setup and rebuilds](docs/development.md#browser-viewer).

## Requirements

Install Rust with Cargo, rustfmt and clippy. Only worker dispatch needs OpenCode v2 with a configured provider. Desktop packets add Visual Studio's **Desktop development with C++** workload, the Windows SDK, CMake and a graphical session. PostgreSQL packets need PostgreSQL 17 and a disposable database of their own. Cargo build scripts and tests run with your account's permissions, so use a checkout and development account that hold no production credentials. A fresh public checkout must build without private material; the [development runbook](docs/development.md) names the tools and environment variables.

## Optional packet runner

Normal work is direct: pick the current delivery step from [ROADMAP.md](ROADMAP.md), build it, run it, and report what you saw. You do not need the runner, a packet or a handoff to start.

The queue is kept as bookkeeping for historical packets and for unattended dispatch when Michael asks for it:

```sh
cargo test --locked --manifest-path tools/Cargo.toml
cargo run --locked --manifest-path tools/Cargo.toml -- status
cargo run --locked --manifest-path tools/Cargo.toml -- postgres exec -- cargo run --locked --manifest-path tools/Cargo.toml -- run 3
```

`run 3` processes up to three pending packets in fresh `opencode run` sessions. The runner checks each handoff and records a local checkpoint. It is not an acceptance gate and does not decide what counts as done; the running app and the owner do.

Use Computer Use for live UI observations and GPUI Kit tests for control behavior. The former browser and Android UI drivers are historical; do not rebuild them.

**Resume (only if a queue run was started):** inspect the last handoff and terminal error, fix the blocker, and run the same command in the same checkout. Partial edits remain on disk. New checkouts keep runner state in `.boardcraft/`; existing `.rustday/` state is reused in place. If both directories exist, the runner stops. After an abrupt shutdown, confirm the runner and worker have exited before removing a stale `lock`. Verification markers are local to this checkout.

**Extending the queue (optional):** register the packet and its inherited outcomes in [tools/src/packets.rs](tools/src/packets.rs) and update the [gate inventory](tools/tests/cutover-gates.tsv). Run the tools tests and check `status` before dispatch. A historical checkpoint does not verify a replacement client.

## Where to read next

| Question | Read |
|---|---|
| What do I build next, and what counts as done? | [ROADMAP.md](ROADMAP.md#delivery-order-owner-direction-2026-11-28) and [docs/workflows.md](docs/workflows.md) |
| How do I build, run and recover the environment? | [docs/development.md](docs/development.md) |
| Where does behaviour live? | [docs/architecture.md](docs/architecture.md) |
| What is verified, and what is still unknown? | [docs/workflows.md](docs/workflows.md) |
| Which document do I update for this change? | [docs/README.md](docs/README.md) |
| How must a worker behave? | [AGENTS.md](AGENTS.md) |
