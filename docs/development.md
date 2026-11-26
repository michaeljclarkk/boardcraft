# Local development

Run commands from the boardcraft checkout. On Windows use x64 MSVC Rust (tested with 1.100), Visual Studio's **Desktop development with C++** workload and a Windows SDK, CMake, Node 22+, OpenCode v2.2.3+ and PostgreSQL 17 client/server binaries. Install Rust via [rustup](https://rustup.rs/) and `rustup component add rustfmt clippy`. For other platforms see [Kit's native prerequisites](https://github.com/longbridge/gpui-kit/blob/v0.11.0/website/docs/installation.md).

```powershell
. .\scripts\dev.ps1               # activate tools in this PowerShell
& .\scripts\postgres.ps1 start    # start/reuse the test cluster; export URL
cargo build --locked -p boardcraft-desktop
$board = cargo run --locked -p boardcraft-desktop --example seed_board
cargo run --locked -p boardcraft-desktop -- $board
```

### Windows inline edit acceptance

Run the native acceptance test from an interactive Windows session. No manual window interaction is required:

```powershell
. .\scripts\dev.ps1
& .\scripts\postgres.ps1 start
cargo test --locked -p boardcraft-desktop --features native-smoke --test windows_smoke -- --nocapture
```

[`windows_smoke`](../crates/desktop/tests/windows_smoke.rs) creates two `Review` rows in `rustday_test` and launches the desktop binary twice with the same board UUID. The [in-app driver](../crates/desktop/src/native_smoke.rs) dispatches pointer and keyboard events to the real Kit window and checks rendered control values, focus, status text and row state. A lock on the second fixture row makes `Saving…` observable before commit. The parent checks IDs and order over a fresh database connection, waits for both processes to exit, and removes the fixture on success. A timeout or missing expected frame fails the test. Normal builds exclude the driver. This checks GPUI event routing and frames, not physical device input or pixel fidelity.

For a manual check, the `seed_board` example above creates a **new** synthetic board containing two rows titled `Review` and prints its UUID. To reopen, reuse `$board`; do not rerun the seed command.

1. Click the second input, select its text and enter spaces only. Press Enter. Check the rejection message, draft and keyboard focus; the first row must still say `Review`.
2. In the second row enter ` Review artwork `, including the surrounding spaces, and click Save. Check that the editor shows `Saved` and `Review artwork` without the spaces. A fast commit may make `Saving…` too brief to observe; record only what you see.
3. Close the window normally. Run the desktop command again with the same `$board`; check that the two rows read `Review`, then `Review artwork`. Close the window before rebuilding on Windows.

With normal tools on PATH the activation script needs no configuration. To place large builds on another drive, create ignored `.private/dev.json` with `toolHome` pointing to a directory containing the rustup `rustup/` and `cargo/` homes. It also places `target/` there. An optional `postgresBin` points to the folder containing `pg_ctl.exe`, `initdb.exe` and `psql.exe`.

### Loopback mobile edit service

With the disposable database active, run `cargo test --locked --workspace --test mobile_edit_service`. Each case seeds two `Review` rows in its own schema and starts the single-board host on `127.0.0.1:0` (OS-assigned port).

To serve the synthetic `$board` from the seed command above, run `cargo run --locked -p boardcraft-mobile-edit-host -- $board`. Use the printed port with `EditClient::new(port)`; press Enter to stop. `GET /boards/{board}` returns ordered `{id,title,revision}` rows. `PUT /boards/{board}/items/{item}/name` takes JSON `{title,revision}` and returns the committed row. Title rejection, revision conflict and missing records return 422, 409 and 404 respectively. A database fault returns 500 without details. This is a local fixture, not a deployed API.

### Android native host

Kit's [v0.11.0 mobile guide](https://github.com/longbridge/gpui-kit/blob/v0.11.0/website/docs/mobile.md) describes limited Android testing of chat controls including Input. Android uses the mobile activity/surface host, not the desktop window entry point. The guide's [example build](https://github.com/longbridge/gpui-mobile/blob/0b882efdac7f524e0bb0b1d4c886b2aa752f9f20/example/build.sh) packages an arm64 Rust library via cargo-ndk and Gradle. Its documented GPUI baseline is 0.3.4; even the [mobile fork revision inspected](https://github.com/longbridge/gpui-mobile/blob/7548ebc4b24f00494af9976af8852c7163ded9ce/Cargo.toml) pins core and Android renderer to 0.3.10. boardcraft's Kit 0.11.0 locks core at **0.3.14**. There is an experimental host, but no demonstrated matching set here.

Local inventory (2026-11-26): JDK 21, SDK platforms/build tools, adb 1.0.41, emulator 36.5.10, x86_64 AVD configurations and an API 36 x86_64 system image are present. No NDK found in the inspected SDK roots; no cargo-ndk in the activated toolchain; only the Windows Rust target is installed. No AVD was booted. To reopen the runtime gate, identify a compatible Kit/core/renderer/mobile revision set, match the NDK build ABI to a dedicated disposable emulator, and observe a rendered Kit input before relying on it. Use build-only packaging and explicit emulator serial selection for any later install/launch: the example's install step uses unqualified adb even in emulator mode. Do not point it at a phone or replace Kit with a WebView.

### Disposable PostgreSQL

The product rename does not migrate PostgreSQL. Keep the existing cluster path, `rustday_admin` and `rustday_test` roles, database name and `RUSTDAY_TEST_DATABASE_URL`. The `rustday_migrations` table and `rustday.board.migrations` advisory lock are shared with earlier binaries so they see the same applied versions and serialize migrations. Do not create a second cluster or rename these identifiers. The ignored `.private/dev.json` tool settings are unchanged. Historical handoffs keep the command and package names actually tested at the time.

The helper uses `127.0.0.1:55437`, `%LOCALAPPDATA%\Rustday\postgres-dev`, SCRAM authentication and a non-superuser test login. It prints the connected database and role, never the URL. Rerun `start` in each fresh shell to export `RUSTDAY_TEST_DATABASE_URL`. Use `& .\scripts\postgres.ps1 stop` when done; stopping preserves data. On startup failure inspect `server.log` in the cluster directory rather than deleting data or stopping another server. On other platforms supply the URL of a dedicated disposable PostgreSQL database directly.

The persistence test requires `RUSTDAY_TEST_DATABASE_URL` to explicitly name the dedicated `rustday_test` database. Its login must be able to create a schema. Each run uses a fresh schema and drops it on success; no fallback URL is used.

```powershell
cargo test --locked --workspace --test postgres_board
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
node --test scripts/run.test.mjs
node scripts/run.mjs status
```

Start one feature session with `node scripts/run.mjs run 1` in the activated shell. The [runner guide](../README.md#run-the-migration) explains restarts.

## Change size

On a fresh clone, enable the repository's staged-size guard with `git config --local core.hooksPath .githooks` (integrate with existing hooks if you use your own). Run `git hook run pre-commit` to check without committing. It counts additions plus deletions across staged text, including lockfiles and docs; binary contents aren't line-counted. The ceiling is 20,000, not a minimum. Split large features at tested boundaries and avoid staging builds, captures or datasets. Cargo defaults to four concurrent jobs; on a constrained machine use `cargo build -j 2` or `-j 1`.
