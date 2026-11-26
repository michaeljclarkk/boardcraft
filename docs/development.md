# Local development

Run commands from the Rustday checkout. On Windows use x64 MSVC Rust (tested with 1.100), Visual Studio's **Desktop development with C++** workload and a Windows SDK, CMake, Node 22+, OpenCode v2.2.3+ and PostgreSQL 17 client/server binaries. Install Rust via [rustup](https://rustup.rs/) and `rustup component add rustfmt clippy`. For other platforms see [Kit's native prerequisites](https://github.com/longbridge/gpui-kit/blob/v0.11.0/website/docs/installation.md).

```powershell
. .\scripts\dev.ps1               # activate tools in this PowerShell
& .\scripts\postgres.ps1 start    # start/reuse the test cluster; export URL
cargo build --locked -p rustday-desktop
$board = cargo run --locked -p rustday-desktop --example seed_board
cargo run --locked -p rustday-desktop -- $board
```

### Windows inline edit acceptance

Run the native acceptance test from an interactive Windows session. No manual window interaction is required:

```powershell
. .\scripts\dev.ps1
& .\scripts\postgres.ps1 start
cargo test --locked -p rustday-desktop --features native-smoke --test windows_smoke -- --nocapture
```

[`windows_smoke`](../crates/desktop/tests/windows_smoke.rs) creates two `Review` rows in `rustday_test` and launches the desktop binary twice with the same board UUID. The [in-app driver](../crates/desktop/src/native_smoke.rs) dispatches pointer and keyboard events to the real Kit window and checks rendered control values, focus, status text and row state. A lock on the second fixture row makes `Saving…` observable before commit. The parent checks IDs and order over a fresh database connection, waits for both processes to exit, and removes the fixture on success. A timeout or missing expected frame fails the test. Normal builds exclude the driver. This checks GPUI event routing and frames, not physical device input or pixel fidelity.

For a manual check, the `seed_board` example above creates a **new** synthetic board containing two rows titled `Review` and prints its UUID. To reopen, reuse `$board`; do not rerun the seed command.

1. Click the second input, select its text and enter spaces only. Press Enter. Check the rejection message, draft and keyboard focus; the first row must still say `Review`.
2. In the second row enter ` Review artwork `, including the surrounding spaces, and click Save. Check that the editor shows `Saved` and `Review artwork` without the spaces. A fast commit may make `Saving…` too brief to observe; record only what you see.
3. Close the window normally. Run the desktop command again with the same `$board`; check that the two rows read `Review`, then `Review artwork`. Close the window before rebuilding on Windows.

With normal tools on PATH the activation script needs no configuration. To place large builds on another drive, create ignored `.private/dev.json` with `toolHome` pointing to a directory containing the rustup `rustup/` and `cargo/` homes. It also places `target/` there. An optional `postgresBin` points to the folder containing `pg_ctl.exe`, `initdb.exe` and `psql.exe`.

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
