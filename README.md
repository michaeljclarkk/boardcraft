# boardcraft

boardcraft is the native Rust port of the internal tool: its entire C# and React/TSX surface rebuilt in Rust with Longbridge GPUI Kit and PostgreSQL. The whole product is the scope.

**The only job is to get boardcraft running 1:1 with the internal tool, in Rust only. Write Rust code, run it, and test the UI with Computer Use and GPUI Kit tests. No extra harnesses or infrastructure.**

Everything here is our own work: our schema, our rules, our controls, our names and our fixtures. This repository contains no other product's code, assets or wording. See [AGENTS.md](AGENTS.md) for the rule that applies to every change.

**First workflow:** Two items on a board are both called “Review”. Rename the second one to “Review artwork”, close the app, then reopen the board. The first row should still say “Review”; the second should keep its new name. The edit targets the item's ID, so matching titles never decide which row changes.

**Available today:** The [board integration test](crates/board/tests/board_edit.rs) verifies the rename in memory. The [PostgreSQL test](crates/board/tests/postgres_board.rs) checks persisted results after reconnecting. The desktop editor's [row-boundary test](crates/desktop/tests/desktop_edit.rs) covers save, failure and retry. The [Windows window test](crates/desktop/tests/windows_smoke.rs) verifies rejection, focus, correction and persistence across app restart. Follow the [development runbook](docs/development.md#windows-inline-edit-acceptance) to reproduce it. The [workflow register](docs/workflows.md) tracks what is verified and what is still open.

Windows and Android are the active targets; iOS follows Android, once Android's checks pass and a Mac is available. Rust owns the rules for board operations, PostgreSQL stores saved board data, and [Longbridge GPUI Kit](https://github.com/longbridge/gpui-kit) supplies the desktop controls.

## Open a board in the browser

From the checkout root on Windows:

```sh
cargo run --locked --manifest-path tools/Cargo.toml -- viewer
```

The Rust viewer builds the GPUI WebAssembly client and opens the board list in your default browser as the local owner. Choose **Review** to try the rename journey, or **Backlog** for an empty board. No username or password is needed. Press Enter in the terminal to stop the host and discard the fixture. See the [UI port checklist](plan/ui-port-checklist.md) and [viewer setup](docs/development.md#browser-viewer).

For unattended viewer checks, run `cargo run --locked --manifest-path tools/Cargo.toml -- viewer --for-seconds 180`. Open the printed URL in Chrome. The fixture expires automatically; no terminal input or process cleanup is needed.

## Requirements

Install Rust with Cargo, rustfmt and clippy. Desktop work adds Visual Studio's **Desktop development with C++** workload, the Windows SDK, CMake and a graphical session. PostgreSQL journeys need PostgreSQL 17 and a disposable database of their own. Cargo build scripts and tests run with your account's permissions, so use a checkout and development account that hold no production credentials. A fresh public checkout must build without private material; the [development runbook](docs/development.md) names the tools and environment variables.

## Developer commands

Work directly from the checkout: pick the current delivery step, build it, run it, and report what you saw. The tools binary only prepares, builds and runs:

```sh
cargo run --locked --manifest-path tools/Cargo.toml -- viewer [--for-seconds N]
cargo run --locked --manifest-path tools/Cargo.toml -- dev -- cargo build --locked -p boardcraft-desktop
cargo run --locked --manifest-path tools/Cargo.toml -- postgres start|stop|exec -- PROGRAM [ARG...]
cargo run --locked --manifest-path tools/Cargo.toml -- prepare-web
```

The queue runner and unattended dispatch were removed. Do not rebuild them.

Use Computer Use for live UI observations and GPUI Kit tests for control behavior. The former browser and Android UI drivers are historical; do not rebuild them.

## Where to read next

| Question | Read |
|---|---|
| What do I build next, and what counts as done? | [ROADMAP.md](ROADMAP.md#delivery-order-owner-direction-2026-11-28) and [docs/workflows.md](docs/workflows.md) |
| How do I build, run and recover the environment? | [docs/development.md](docs/development.md) |
| Where does behaviour live? | [docs/architecture.md](docs/architecture.md) |
| What is verified, and what is still unknown? | [docs/workflows.md](docs/workflows.md) |
| Which document do I update for this change? | [docs/README.md](docs/README.md) |
| How must a worker behave? | [AGENTS.md](AGENTS.md) |
