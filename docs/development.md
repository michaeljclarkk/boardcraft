# Local development

Run commands from the checkout root. On Windows use x64 MSVC Rust, Visual Studio's **Desktop development with C++** workload, a Windows SDK and CMake. Install Cargo via [rustup](https://rustup.rs/) and add `rustfmt` and `clippy`. Preparing the workspace uses Git, tar and (on a cache miss) curl. Database journeys need PostgreSQL 17 client/server binaries. Only queue dispatch requires OpenCode with a configured provider. See [browser viewer](#browser-viewer) for the web toolchain.

`dev -- PROGRAM [ARG...]` prepares the workspace dependency and runs one child with the configured tool paths. On Windows, `postgres exec -- PROGRAM [ARG...]` also starts or reuses the local cluster and gives that child the disposable database URL. Neither changes the calling shell. For the examples below, define these PowerShell shortcuts once, or use their full Cargo commands:

```powershell
function bc { cargo run --locked --manifest-path tools/Cargo.toml -- @args }
function bcdb { bc postgres exec -- @args }
bc dev -- cargo build --locked -p boardcraft-desktop
$board = bcdb cargo run --locked -p boardcraft-desktop --example seed_board
$env:BOARDCRAFT_LOCAL_ACTOR = [guid]::NewGuid().ToString() # once per synthetic actor; reuse on reopen
bcdb cargo run --locked -p boardcraft-desktop -- $board
```

Only Computer Use and GPUI Kit tests verify UI. Use Computer Use for the live steps below; record the action and visible outcome. If an essential step cannot be observed, keep it pending. Builds, host requests and independent database reads answer different questions.

### Windows inline edit acceptance

To read a protected board from a running loopback host, use `boardcraft-desktop connect <loopback-port> <board-uuid>`. With no session configured, enter the existing account's identifier, choose **Next**, then enter the password and choose **Sign in**. Closing the window drops that interactive session from memory. The single-UUID command above opens the developer editor.

A Windows launcher that already holds an issued session can start the built desktop executable with a child-only environment (PowerShell 7+ below). `$sessionOrigin` is the origin associated with the token at issuance, **not** a value derived from the requested board or destination. It must match `http://127.0.0.1:<port>` exactly, without a trailing slash. Use a client process separate from the database-configured host.

```powershell
# $desktopExe is the built executable's path; the launcher holds the session pair in memory.
$child = [System.Diagnostics.ProcessStartInfo]::new($desktopExe)
$child.UseShellExecute = $false
$child.ArgumentList.Add('connect')
$child.ArgumentList.Add([string]$port)
$child.ArgumentList.Add([string]$board)
$child.Environment['BOARDCRAFT_SESSION_ORIGIN'] = $sessionOrigin
$child.Environment['BOARDCRAFT_SESSION_TOKEN'] = $sessionToken
$null = [System.Diagnostics.Process]::Start($child)
```

An incomplete, malformed or wrong-origin configuration opens interactive sign-in without contacting the host. A matching token must resolve and have access to the selected board. Revocation returns to sign-in; **Retry loading** after a connection failure uses the same session in memory. The desktop does not save credentials or update its environment.

On a connected board with edit access, double-click a name or focus it and press F2. Escape cancels; Enter or moving focus away submits a changed name. `Saving…` shows the submitted spelling until the host returns the committed name. Viewer names are disabled. If a Member loses write access during editing, the window reports the denial and checks read access again. Connected edits do not use the developer recovery directory.

Choose **Log out** on either an editable or read-only board. After the host acknowledges, the same window shows an empty **Username or email** field. Other sessions stay signed in. If the window shows **Could not log out**, it keeps the board available; click **Log out** to retry. This message does not prove the session is still valid. A fresh launch with the old, revoked session will ask you to sign in.

For failed-name recovery, reuse the same actor UUID, board UUID and local directory across launches. `BOARDCRAFT_LOCAL_ACTOR` is an explicit development context, not sign-in or board authorization. Without it, failure actions work only in the live window. Local files default to `%LOCALAPPDATA%\boardcraft\name-recovery`; override with `BOARDCRAFT_RECOVERY_DIR` (an absolute directory private to the local user). The graphical tests supply their own temporary directories and actors. No database configuration goes into the recovery file.

To check recovery manually, reject a blank name, close without dismissing and relaunch. The board should show the accepted label, failure reason, `Retry change` and `Dismiss`. Dismiss and reopen: the failure should be gone. If local recovery cannot be read, the board still shows its accepted names and a warning. New failures last only in that window. The unreadable file stays untouched; there is no in-app repair or discard action.

To check an **unsubmitted** draft, reuse the same actor, board and directory. Double-click the second name, replace it (or clear it), then close the window with the input still focused. Relaunch: the changed text should be in the focused input. Escape should show the latest accepted title and clear only that draft. Opening and closing an untouched input should leave nothing to restore. Without an explicit actor, drafts last only in the window.

Run the ordinary native acceptance tests serially in a graphical Windows session. They exclude the two board-link clipboard journeys:

```powershell
bcdb cargo test --locked -p boardcraft-desktop --features native-smoke --test windows_smoke -- --test-threads=1 --nocapture
```

The board-link journeys replace the system clipboard and paste through Kit inputs. Run them only in a dedicated graphical session with a fixture-only clipboard or during an agreed foreground and clipboard handoff for that run. Do not inspect or save previous clipboard contents. With that setup, use `cargo test --locked -p boardcraft-desktop --features attended-clipboard --test windows_smoke -- board_link --ignored --test-threads=1`.

The existing [`windows_smoke`](../crates/desktop/tests/windows_smoke.rs) tests exercise pointer and keyboard events on Kit controls. The name-edit fixture has two `Review` rows (IDs 42 and 17); fresh database connections check their identity, order, spelling and revisions. Submission counts distinguish an unchanged exit from a no-op transaction. These checks cover GPUI event routing, not physical input or pixel fidelity.

For a manual check, the `seed_board` example above creates a **new** synthetic board containing two rows titled `Review` and prints its UUID. To reopen, reuse `$board`; do not rerun the seed command.

1. Click the second name. It should have focus but no editor. Press F2 or double-click it, select its text and enter spaces only. Press Enter. Both labels should read `Review`, with focus on the second. Click `Retry change`; the same rejection should return. Click `Dismiss` to clear it.
2. Double-click the second `Review`. Replace it with ` Review artwork `, including the surrounding spaces, and press Enter. Check that the editor closes and the row shows `Saved` and `Review artwork` without the spaces. A fast commit may make `Saving…` too brief to observe; record only what you see.
3. Close the window normally. Run the desktop command again with the same `$board`; check that the two rows read `Review`, then `Review artwork`. Close the window before rebuilding on Windows.

With tools on PATH no configuration file is needed. To place child builds on another drive, create ignored `.private/dev.json` with `toolHome` pointing to a directory containing the rustup `rustup/` and `cargo/` homes. It also places `target/` there unless `CARGO_TARGET_DIR` is set. An optional `postgresBin` points to the folder containing `pg_ctl.exe`, `initdb.exe` and `psql.exe`.

### Loopback mobile edit service

With the disposable database active, run `cargo test --locked --workspace --test mobile_edit_service`. Each case seeds two `Review` rows in its own schema and starts the single-board host on `127.0.0.1:0` (OS-assigned port).

Run `cargo test --locked --workspace --test loopback_board_access` for the protected-host fixture. It signs in generated accounts in memory and supplies `EditClient::with_bearer` with the issued session. Starting the host with a board UUID or seeding a desktop board grants no access. Do not put a bearer on a command line or in a URL.

`GET /boards/{board}` returns ordered `{id,title,revision}` rows. `PUT /boards/{board}/items/{item}/name` takes JSON `{title,revision}` and returns the committed row. Both resolve the session and check the board grant on every call. Missing/rejected session: 401 (sign in); denied access: 403 (do not retry unchanged permissions); unavailable access check: 503 (retry later). Title rejection, revision conflict, missing records and unexpected board faults retain 422, 409, 404 and 500 respectively. This is a local fixture, not a deployed API.

The GET response header `boardcraft-board-access` describes access at read time: only a single `edit` value enables Windows name editing. `read`, a missing header or an unknown value leaves the board read-only. The PUT does not take a permission hint.

For multi-board discovery, start `Host::start_with_navigation(config, target)` and launch `boardcraft-desktop browse <loopback-port>`. Use the same exact-origin session configuration or interactive sign-in as `connect`. `EditClient::list_boards` calls `GET /navigation/boards` and returns `{id, workspace_id, name}` summaries in global board order. Click a board to read its items through `GET /navigation/boards/{id}`. This view is read-only. **Back to boards** loads the list again. After **Destination unavailable**, return to the list; after **No boards**, use **Refresh** to check for newly visible boards. A service fault offers **Retry board list**; session rejection requires sign-in. Ordinary `Host::start` serves neither navigation route. In both modes the direct `/boards/{id}` GET/PUT routes remain restricted to `target`.

In Windows browse, open a board and choose **Share** for its Board address. **Copy link** copies it to the clipboard. **Open in new window** reads that board independently. A recipient pastes the address into their own browse window and chooses **Open board link**. The connected origin must match and the recipient must already have access. After a denial, they can submit the same address again if access changes.

### Browser viewer

Install the [web crate's pinned nightly and wasm target](../crates/web/rust-toolchain.toml), [Trunk and matching wasm-bindgen](../crates/web/Trunk.toml). Launch the [Rust viewer](../crates/mobile-edit-host/examples/web_board_view.rs) in a terminal:

```powershell
rustup toolchain install nightly-2026-10-31 --target wasm32-unknown-unknown
cargo install trunk --version 0.22.2 --locked
cargo install wasm-bindgen-cli --version 0.2.130 --locked
cargo run --locked --manifest-path tools/Cargo.toml -- viewer
```

The viewer opens the board list in your default browser as the fixed local owner. Choose **Review** (two Work groups and an empty Reserve) or **Backlog** (no items). The terminal also prints the loopback URL for another browser. Refreshing or opening a new tab uses that same identity. To verify persistence across client restarts, close the tab and reopen its board URL while the host is still running. Press Enter in the terminal to stop the host and drop its fixture. After a source change, stop the old viewer and rerun `viewer`: the host snapshots assets at startup.

For a live check, use Computer Use to rename the second of the two **Review** items, reopen the board and compare both names. Use GPUI Kit tests for control behavior. For authorization, scope, commits and rollback run the existing host tests against the disposable database:

```powershell
bcdb cargo test --locked -p boardcraft-mobile-edit-host --test loopback_board_access
```

The `pending` browser outcomes in the [runner inventory](../tools/tests/cutover-gates.tsv) are observations still needed on the current client, not test targets. Do not recreate a browser driver, use OCR or CDP, or substitute another GUI harness. The earlier browser suites are historical.

### Shared component work

When an interaction is missing, add it to `crates/boardcraft-ui` rather than to a client. Each control keeps a runnable example and interaction tests, and must build for native and wasm:

```powershell
cargo test --locked -p boardcraft-ui
cargo run --locked -p boardcraft-ui --example heading_drag
cargo +nightly-2026-10-31 build --locked -p boardcraft-ui --target wasm32-unknown-unknown
```

Record the control in [the register](gpui-components.md). Do not embed HTML/CSS/JS/React/TSX, a webview or a DOM overlay to avoid the shared work.

### Android native host

Android uses Kit `=0.11.0` with the [pinned mobile host and compatibility patch](../android/patches/README.md). A packaged APK is not live UI evidence. The frame, edit, browse and cross-client reopen drivers are retired. For a live check use Computer Use on the visible dedicated emulator; keep authorization and store readbacks separate.

### Android emulator edit gate

Planning preflight (2026-11-26): the installed API 36.1 x86_64 image booted on a fresh, isolated AVD with WHPX. The Rust target plus NDK r27c API 28 linker produced a runnable Android executable. A temporary read-only caller of the existing `EditClient` read through `adb reverse` to the production 04 host; PostgreSQL independently returned the same two ordered IDs, titles and revisions. The mapping, device probe and schema were removed. This did not exercise a save or any Kit UI.

For the non-UI packet 05 probe, install `rustup target add x86_64-linux-android`. Set `ANDROID_HOME` to the SDK with the image and emulator (SDK roots can differ); put **that SDK's** `platform-tools` on PATH. Set `CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER` to the NDK's `toolchains/llvm/prebuilt/windows-x86_64/bin/x86_64-linux-android28-clang.cmd`. NDK and emulator need not be in the same SDK. No cargo-ndk, Gradle, signing or network deployment is needed here. Use existing licenses only.

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

Allow up to three minutes for boot. In this same environment, use `adb -s $env:RUSTDAY_ANDROID_SERIAL` for device commands. Check `emu avd name`, `shell getprop sys.boot_completed` and `shell getprop ro.product.cpu.abi` for the exact name, `1` and `x86_64`. Keep both processes running through the probe. Afterwards, confirm the AVD name before issuing `emu kill`, and confirm the private server port and listener ownership before using `adb -P 5039 kill-server`. Never stop the shared daemon.

On Windows, platform-tools 36.0.0 and 37.0.3 both aborted on the second `reverse --no-rebind tcp:0` fixture: the device allocated a new listener, but the server still associated literal `tcp:0` with the first host port. After removing the actual listener and executable, the test checks that there are no remaining mappings, reconnects **only the selected transport**, repeats that emulator's registration on the private loopback server, and revalidates the AVD. No probe is replayed. To capture a server failure, start the private server in a second identically activated shell with `adb -L tcp:5039 nodaemon server` instead of `start-server` and inspect its stderr alongside Windows Application Error events.

Packet 05 commands: the host test signs in a fixture Member with access and pipes its session to each probe via non-PTY stdin. No bearer is in adb arguments, the executable or a device file.

```powershell
bc dev -- cargo build --locked -p boardcraft-mobile-edit-client --bin android-edit-probe --target x86_64-linux-android
bcdb cargo test --locked -p boardcraft-mobile-edit-host --features android-smoke --test android_emulator_edit -- --test-threads=1
```

Use the cross-target debug executable from Cargo's target directory, never a host build. The runner gives each adb preflight 10 seconds and the focused host suite 10 minutes including compilation. The test itself must time out and reap hung adb children, parse successful **device** results, and fail if prerequisites disappear. It must not boot, wipe or stop any emulator. [05 acceptance](../packets/05-android-emulator-edit.md) specifies both journeys.

### Historical Android Kit journeys

The frame, edit and cross-client reopen handoffs describe earlier runs. For current acceptance, use Computer Use to observe the second item's editor, blank-name rejection, correction and save, then compare the resumed draft and the cold-reopened board with independent stored rows. A headless device probe cannot verify any of these screens.

### Disposable PostgreSQL

The product rename does not migrate PostgreSQL. Keep the existing cluster path, `rustday_admin` and `rustday_test` roles, database name and `RUSTDAY_TEST_DATABASE_URL`. The `rustday_migrations` table and `rustday.board.migrations` advisory lock are shared with earlier binaries so they see the same applied versions and serialize migrations. Do not create a second cluster or rename these identifiers. The ignored `.private/dev.json` tool settings are unchanged. Historical handoffs keep the command and package names actually tested at the time.

The helper uses `127.0.0.1:55437`, `%LOCALAPPDATA%\Rustday\postgres-dev`, SCRAM authentication and a non-superuser test login. Use `bcdb` to pass the database URL to the child and `bc postgres stop` when done; stopping preserves data. On startup failure inspect `server.log` in the cluster directory rather than deleting data or stopping another server. On other platforms supply the URL of a dedicated disposable PostgreSQL database directly.

The persistence test requires `RUSTDAY_TEST_DATABASE_URL` to explicitly name the dedicated `rustday_test` database. Its login must be able to create a schema. Each run uses a fresh schema and drops it on success; no fallback URL is used.

```powershell
bcdb cargo test --locked --workspace --test postgres_board
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
bcdb cargo test --locked --workspace
cargo test --locked --manifest-path tools/Cargo.toml
bc status
```

Normal development does not use the runner. Build and test directly, or start a feature session only when Michael asks for queue dispatch with `bcdb cargo run --locked --manifest-path tools/Cargo.toml -- run 1`. The [optional runner guide](../README.md#optional-packet-runner) explains restarts.

## Change size

On a fresh clone, enable the repository's staged-size guard with `git config --local core.hooksPath .githooks` (integrate with existing hooks if you use your own). Run `git hook run pre-commit` to check without committing. It counts additions plus deletions across staged text, including lockfiles and docs; binary contents aren't line-counted. The ceiling is 20,000, not a minimum. Split large features at tested boundaries and avoid staging builds, captures or datasets. Cargo defaults to four concurrent jobs; on a constrained machine use `cargo build -j 2` or `-j 1`.
