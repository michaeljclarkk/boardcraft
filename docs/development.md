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

For the native app, identify a compatible Kit/core/renderer/mobile revision set and observe a rendered Kit input before relying on it. Use build-only packaging and explicit emulator serial selection for any later install/launch: the example's install step uses unqualified adb even in emulator mode. Do not point it at a phone or replace Kit with a WebView.

### Android emulator edit gate

Planning preflight (2026-11-26): the installed API 36.1 x86_64 image booted on a fresh, isolated AVD with WHPX. The Rust target plus NDK r27c API 28 linker produced a runnable Android executable. A temporary read-only caller of the existing `EditClient` read through `adb reverse` to the production 04 host; PostgreSQL independently returned the same two ordered IDs, titles and revisions. The mapping, device probe and schema were removed. This did not exercise a save or any Kit UI.

On Windows, activate `scripts/dev.ps1` and start `scripts/postgres.ps1 start` as above. Install `rustup target add x86_64-linux-android`. Set `ANDROID_HOME` to the SDK with the image and emulator (SDK roots can differ); put **that SDK's** `platform-tools` on PATH. Set `CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER` to the NDK's `toolchains/llvm/prebuilt/windows-x86_64/bin/x86_64-linux-android28-clang.cmd`. NDK and emulator need not be in the same SDK. No cargo-ndk, Gradle, signing or network deployment is needed here. Use the SDK manager with existing licenses only; stop on a new license prompt.

Create `Rustday_Edit_X64` **once** in a separate `ANDROID_AVD_HOME` on a volume with space; use the installed x86_64 image, no account or personal data. Example: `avdmanager create avd -n Rustday_Edit_X64 -k "system-images;android-36.1;google_apis_playstore;x86_64"`. Do not overwrite an existing AVD.

Use an exclusive loopback adb server. Choose three unused ports: a non-default server port and an adjacent even/odd console/transport pair **outside** other servers' emulator scan ranges. The emulator registers itself with the private server at startup; start the server first. Example (Windows PowerShell, in the activated shell):

```powershell
if (Get-NetTCPConnection -State Listen | Where-Object LocalPort -in 5039,5682,5683) { throw 'Choose unused ports' }
$env:ANDROID_ADB_SERVER_PORT = '5039'   # emulator startup notification
$env:ADB_SERVER_SOCKET = 'tcp:5039'     # clients and child processes
$env:ADB_USB = '0'; $env:ADB_EMU = '0'; $env:ADB_MDNS = '0'
$env:RUSTDAY_ANDROID_SERIAL = 'emulator-5682'
adb -L tcp:5039 start-server
if ($LASTEXITCODE) { throw 'Private adb startup failed' }
Start-Process emulator -ArgumentList '-avd Rustday_Edit_X64 -ports 5682,5683 -no-window -no-audio -no-snapshot -gpu swiftshader_indirect -no-boot-anim'
```

Allow up to three minutes for boot. In this same environment, use `adb -s $env:RUSTDAY_ANDROID_SERIAL` for device commands. Check `emu avd name`, `shell getprop sys.boot_completed` and `shell getprop ro.product.cpu.abi` for the exact name, `1` and `x86_64`. Keep both processes running through `node scripts/run.mjs run 1`. Afterwards, confirm the AVD name before issuing `emu kill`, and confirm the private server port and listener ownership before using `adb -P 5039 kill-server`. Never stop the shared daemon.

On Windows, platform-tools 36.0.0 and 37.0.3 both aborted on the second `reverse --no-rebind tcp:0` fixture: the device allocated a new listener, but the server still associated literal `tcp:0` with the first host port. After removing the actual listener and executable, the test checks that there are no remaining mappings, reconnects **only the selected transport**, repeats that emulator's registration on the private loopback server, and revalidates the AVD. No probe is replayed. To capture a server failure, start the private server in a second identically activated shell with `adb -L tcp:5039 nodaemon server` instead of `start-server` and inspect its stderr alongside Windows Application Error events.

Packet 05 commands (the worker supplies the probe and feature-gated integration test):

```powershell
cargo build --locked -p boardcraft-mobile-edit-client --bin android-edit-probe --target x86_64-linux-android
cargo test --locked -p boardcraft-mobile-edit-host --features android-smoke --test android_emulator_edit -- --test-threads=1
```

Use the cross-target debug executable from Cargo's target directory, never a host build. The runner gives each adb preflight 10 seconds and the focused host suite 10 minutes including compilation. The test itself must time out and reap hung adb children, parse successful **device** results, and fail if prerequisites disappear. It must not boot, wipe or stop any emulator. [05 acceptance](../ROADMAP.md#05-android-emulator-edit) specifies both journeys.

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
