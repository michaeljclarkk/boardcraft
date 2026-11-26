# Local development

Run commands from the boardcraft checkout. On Windows use x64 MSVC Rust (tested with 1.100), Visual Studio's **Desktop development with C++** workload and a Windows SDK, CMake, Node 22+, OpenCode v2.2.3+ and PostgreSQL 17 client/server binaries. Install Rust via [rustup](https://rustup.rs/) and `rustup component add rustfmt clippy`. For other platforms see [Kit's native prerequisites](https://github.com/longbridge/gpui-kit/blob/v0.11.0/website/docs/installation.md).

```powershell
. .\scripts\dev.ps1               # activate tools in this PowerShell
& .\scripts\postgres.ps1 start    # start/reuse the test cluster; export URL
cargo build --locked -p boardcraft-desktop
$board = cargo run --locked -p boardcraft-desktop --example seed_board
$env:BOARDCRAFT_LOCAL_ACTOR = [guid]::NewGuid().ToString() # once per synthetic actor; reuse on reopen
cargo run --locked -p boardcraft-desktop -- $board
```

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

Run the native acceptance test from an interactive Windows session. No manual window interaction is required:

```powershell
. .\scripts\dev.ps1
& .\scripts\postgres.ps1 start
cargo test --locked -p boardcraft-desktop --features native-smoke --test windows_smoke -- --test-threads=1 --nocapture
```

The five developer [`windows_smoke`](../crates/desktop/tests/windows_smoke.rs) cases create two `Review` rows (IDs 42 and 17) in a fresh `rustday_test` schema. The [in-app driver](../crates/desktop/src/native_smoke.rs) dispatches pointer and keyboard events to the displayed controls. It waits for the parent to check both IDs, order, spelling and revisions on a fresh connection before advancing. A submission count distinguishes an unchanged exit from a no-op transaction. The recovery case rejects `Coral` with a fixture-only constraint, removes it and locks the second row. While `Saving…` is visible the display includes the submitted padding and the stored title is still `Review`. The reopen case checks the committed spelling in a second process. The connected login case instead starts the shared loopback host with `Review sketch` and `Ink`; the parent checks the session count, account and board independently. Normal builds exclude the drivers. This checks GPUI event routing and frames, not physical device input or pixel fidelity.

For a manual check, the `seed_board` example above creates a **new** synthetic board containing two rows titled `Review` and prints its UUID. To reopen, reuse `$board`; do not rerun the seed command.

1. Click the second name. It should have focus but no editor. Press F2 or double-click it, select its text and enter spaces only. Press Enter. Both labels should read `Review`, with focus on the second. Click `Retry change`; the same rejection should return. Click `Dismiss` to clear it.
2. Double-click the second `Review`. Replace it with ` Review artwork `, including the surrounding spaces, and press Enter. Check that the editor closes and the row shows `Saved` and `Review artwork` without the spaces. A fast commit may make `Saving…` too brief to observe; record only what you see.
3. Close the window normally. Run the desktop command again with the same `$board`; check that the two rows read `Review`, then `Review artwork`. Close the window before rebuilding on Windows.

With normal tools on PATH the activation script needs no configuration. To place large builds on another drive, create ignored `.private/dev.json` with `toolHome` pointing to a directory containing the rustup `rustup/` and `cargo/` homes. It also places `target/` there. An optional `postgresBin` points to the folder containing `pg_ctl.exe`, `initdb.exe` and `psql.exe`.

### Loopback mobile edit service

With the disposable database active, run `cargo test --locked --workspace --test mobile_edit_service`. Each case seeds two `Review` rows in its own schema and starts the single-board host on `127.0.0.1:0` (OS-assigned port).

Run `cargo test --locked --workspace --test loopback_board_access` for the protected-host fixture. It signs in generated accounts in memory and supplies `EditClient::with_bearer` with the issued session. Starting the host with a board UUID or seeding a desktop board grants no access. Do not put a bearer on a command line or in a URL.

`GET /boards/{board}` returns ordered `{id,title,revision}` rows. `PUT /boards/{board}/items/{item}/name` takes JSON `{title,revision}` and returns the committed row. Both resolve the session and check the board grant on every call. Missing/rejected session: 401 (sign in); denied access: 403 (do not retry unchanged permissions); unavailable access check: 503 (retry later). Title rejection, revision conflict, missing records and unexpected board faults retain 422, 409, 404 and 500 respectively. This is a local fixture, not a deployed API.

The GET response header `boardcraft-board-access` describes access at read time: only a single `edit` value enables Windows name editing. `read`, a missing header or an unknown value leaves the board read-only. The PUT does not take a permission hint.

For multi-board discovery, explicitly use `Host::start_with_navigation(config, target)`. `EditClient::list_boards` calls `GET /navigation/boards` with the current session and returns `{id, workspace_id, name}` summaries in global board order. An empty list is a successful read. Session rejection requires sign-in; an unavailable access store can be retried with the same session. Ordinary `Host::start` leaves the list route disabled. Both host modes serve item reads and writes only for `target`.

### Android native host

Kit's [v0.11.0 mobile guide](https://github.com/longbridge/gpui-kit/blob/v0.11.0/website/docs/mobile.md) describes limited Android testing of chat controls including Input. Android uses the mobile activity/surface host, not the desktop window entry point. The guide's [example build](https://github.com/longbridge/gpui-mobile/blob/0b882efdac7f524e0bb0b1d4c886b2aa752f9f20/example/build.sh) packages an arm64 Rust library via cargo-ndk and Gradle. Its documented GPUI baseline is 0.3.4; even the [mobile fork revision inspected](https://github.com/longbridge/gpui-mobile/blob/7548ebc4b24f00494af9976af8852c7163ded9ce/Cargo.toml) pins core and Android renderer to 0.3.10. boardcraft's Kit 0.11.0 locks core at **0.3.14**. There is an experimental host, but no demonstrated matching set here.

The [06 frame gate](#android-kit-frame-gate) uses a matching **Android-only** release set. Use build-only packaging and explicit emulator serial selection: the example's install step uses unqualified adb even in emulator mode.

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

Packet 05 commands: the host test signs in a fixture Member with access and pipes its session to each probe via non-PTY stdin. No bearer is in adb arguments, the executable or a device file.

```powershell
cargo build --locked -p boardcraft-mobile-edit-client --bin android-edit-probe --target x86_64-linux-android
cargo test --locked -p boardcraft-mobile-edit-host --features android-smoke --test android_emulator_edit -- --test-threads=1
```

Use the cross-target debug executable from Cargo's target directory, never a host build. The runner gives each adb preflight 10 seconds and the focused host suite 10 minutes including compilation. The test itself must time out and reap hung adb children, parse successful **device** results, and fail if prerequisites disappear. It must not boot, wipe or stop any emulator. [05 acceptance](../ROADMAP.md#05-android-emulator-edit) specifies both journeys.

### Android Kit frame gate

Planning experiment (2026-11-26), compiled and installed on `Rustday_Edit_X64`, API 36.1/x86_64:

- With desktop Kit `=0.11.0` and the unmodified [Longbridge mobile host at `7548ebc`](https://github.com/longbridge/gpui-mobile/tree/7548ebc4b24f00494af9976af8852c7163ded9ce), Cargo refused to resolve `gpui-pre` (`=0.3.14` versus `=0.3.10`). Repinning the host alone in scratch built the 0.3.14 WGPU crate but failed at `android/window.rs:1405`: `WgpuContext::new` now requires a surface configuration. Don't repin or downgrade the desktop workspace for 06.
- An independent scratch `cdylib` using **`gpui-kit = "=0.8.1"`** (`default-features = false`, `features = ["component"]`), unmodified `gpui-pre-mobile` at the above git SHA (`default-features = false`, alias `gpui-mobile`) and `android-activity` 0.6 with `native-activity` resolved **one** core and renderer, both 0.3.10. `cargo build --locked --lib --target x86_64-linux-android` linked. No vendored renderer, upstream demos, fonts or Java screens needed. Keep the Android lockfile and run `cargo fmt --manifest-path android/Cargo.toml -- --check` as well as workspace fmt.
- The signed x86_64 APK used the system `android.app.NativeActivity` with `android.app.lib_name` set to the cdylib stem. Follow the [upstream Android entry point](https://github.com/longbridge/gpui-mobile/blob/7548ebc4b24f00494af9976af8852c7163ded9ce/example/src/lib.rs#L83-L128): `android_main` initializes `jni::init_platform(&app)` and runs `Application::with_platform` with the shared platform. After `InitWindow`, call `gpui_kit::init`, `cx.open_window` and wrap the view in `component::Root`. The platform owns the native window and WGPU surface; Kit owns the pixels and hit testing.
- Stock Vulkan initialization crashed in the emulator's `vulkan.ranchu.so` (`vk_common_SetDebugUtilsObjectNameEXT`) with both SwiftShader and host GPU. Disabling debug assertions **only for `wgpu-types`** got past adapter selection on host Vulkan, but first submission crashed in `ResourceTracker::on_vkQueueSubmit`. A successful `am start -W` and “window opened” log preceded failure. They are not frame evidence.
- **Working configuration:** boot this AVD with `-gpu host` instead of `-gpu swiftshader_indirect`; set `gpui_mobile::android::host::set_gpu_backend(GpuBackend::Gl)` before platform initialization. Tested with `[profile.dev.package.wgpu-types] debug-assertions = false`, native library debug info omitted, debuggable APK. WGPU selected the emulator OpenGL ES translator. Windows OCR read `Native sample` from a fresh `adb -s <serial> exec-out screencap -p`; tapping the Kit button at the recognized position (311,191 on that run) produced `Touch received` in a second capture. **Only the generic button was prototyped**; no board, Input, save or IME acceptance inferred.

Use the [05 private server/AVD procedure](#android-emulator-edit-gate) with the host GPU option above. Windows PowerShell 5.1 with English Windows OCR, JDK, SDK platform `android-35`, build-tools `35.0.0`, NDK r27c and the Rust x86_64 Android target were used. In addition to the 05 linker, set `CC_x86_64_linux_android`, `CXX_x86_64_linux_android` and `AR_x86_64_linux_android` to the NDK's `x86_64-linux-android28-clang.cmd`, `x86_64-linux-android28-clang++.cmd` and `llvm-ar.exe` respectively. Use Cargo metadata's target directory for the `.so`. Package it under `lib/x86_64/` (the simple manifest can set `android:hasCode="false"`, `android:extractNativeLibs="true"`, `android:debuggable="true"`); `aapt package`, `zipalign -P 16` and `apksigner sign` with a local debug key worked without Gradle. Never publish keys or APKs. A fresh checkout must not depend on a scratch clone or developer paths.

Worker build contract: `android/build-debug.ps1 -OutputApk <new absolute path>` must build the locked Rust app from current sources, sign the debug fixture, propagate failures and leave that APK at the requested path. No adb or emulator startup. Test on the already booted, unlocked, exclusive AVD:

```powershell
node scripts/android-kit-frame.mjs       # build, install, screen → tap → screen, uninstall
node --test scripts/run.test.mjs        # gate regressions, not emulator evidence
```

The [frame gate](../scripts/android-kit-frame.mjs) checks package ID/ABI/debug flag before install and refuses to replace an existing package. It observes the foreground activity, captures PNG bytes directly, recognizes ordered text and derives touch coordinates from the first frame. It fails on missing frames, wrong order, stale frames and timeouts. Do not use a static screenshot or draw a second copy of the Input value for OCR. Keep observed strings legible and away from the system bars. Keyboard and lifecycle need their own checks; in particular the mobile host [documents the NativeActivity recreation limitation and host-driven alternative](https://github.com/longbridge/gpui-mobile/blob/7548ebc4b24f00494af9976af8852c7163ded9ce/src/android/host.rs#L1-L51). If blocked on another renderer/driver, preserve the error, package/version tuple and actual last frame; independent source work is the Windows display-to-editor transition (double-click/F2 → draft → Escape restores the accepted name, no revision change). Do not pass 06 with that separate interaction.

### Android Kit edit gate

Connected launches supply only the board UUID and loopback port. The driver generates a fixture password in memory, sends it to `kit_edit_fixture` on stdin before startup JSON, and enters it through the masked Kit input. The `rejected` and `signed-in` checkpoints verify zero and one sessions respectively, along with both original rows. A cold launch resolves the stored session for that port. The no-arguments frame gate needs no credentials or host.

Use the frame gate's dedicated AVD and host GPU setup, with `rustday_test` active in the operator shell. Use a JDK with `javac --release 8` support. The build packages the Keystore helper and `GpuiInputActivity.java` from the locked mobile host checkout alongside the Rust library. The activity's transparent input connection forwards keyboard edits to the focused GPUI handler.

```powershell
node scripts/android-kit-frame.mjs       # no extras: original offline fixture
node --test scripts/run.test.mjs
node scripts/android-kit-edit.mjs        # live Kit input and independent store readbacks
node scripts/android-kit-edit.mjs --logout # disconnect, edit after failure, log out and sign in again
```

The edit gate starts [`kit_edit_fixture`](../crates/mobile-edit-host/examples/kit_edit_fixture.rs) against a fresh schema. Only a board UUID and reversed loopback port go into the activity intent. The fixture's stdin replies reflect new store connections; EOF stops the host and drops the schema. Record the live gate outcomes in the handoff before runner verification.

### Cross-client reopen gate

Run `node scripts/cross-client-reopen.mjs` in a graphical Windows session with the Android Kit edit prerequisites above. The gate continues the live edit on the same fixture: it backgrounds and resumes the unsaved `Amber` editor, opens the Windows binary with `--smoke-delivery` and the fixture schema, then force-stops and reopens the Android package. The Windows driver reports the two visible name button labels with their bound IDs and checks that neither editor is mounted; `saved` readbacks check the committed titles and revisions separately.

For packet 17, `node scripts/cross-client-reopen.mjs --restore` checks new Android processes on two device ports, the retry-only screen after disconnection, and offline cold launches after rejection and logout. The ordinary delivery journey revokes its session before testing cold reauthentication. Both use fresh PostgreSQL connections for the accepted rows; the restore checkpoints compare the session verifier in memory.

For a storage fault check, use a separate disposable install on the dedicated AVD. With the process stopped, truncate only the selected origin's synthetic record in `no_backup` without displaying its contents. Relaunch: expect `Could not read stored session` and `Retry storage`, with no board or login form. An unreadable record should give the same result; removing it and tapping `Retry storage` should show identifier entry without contacting the host. Do not use this fault injection during the continuity gate.

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
