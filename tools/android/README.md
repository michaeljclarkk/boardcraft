# Android operator tools

This standalone Rust workspace owns build-only APK packaging, pinned mobile-source
preparation and read-only preflight of an explicitly selected disposable emulator.
It is not an application dependency and contains no UI driver or OCR implementation.

From the checkout root:

```text
cargo run --locked --manifest-path tools/android/Cargo.toml -- prepare-mobile
cargo run --locked --manifest-path tools/android/Cargo.toml -- test-prepare-mobile
cargo run --locked --manifest-path tools/android/Cargo.toml -- build-debug --output-apk <new-absolute-path>
cargo run --locked --manifest-path tools/android/Cargo.toml -- check-device
```

`--root <checkout>` is available for relocated binaries. `build-debug` does not
contact adb, install a package, start an emulator or use a configured signing key.
It generates and destroys a per-build debug key, verifies the signature and refuses
to overwrite the output. Packaging uses the installed JDK and SDK tools; only the
unmodified, pinned upstream IME activity is compiled as Java. No displaced script or
authored Java source is embedded in the Rust implementation.

## Prerequisites and ownership

- Set `ANDROID_HOME`, `JAVA_HOME` and the x86_64 Android Rust/NDK compiler variables
  described in [Android setup](../../android/README.md). Packaging expects Android platform 35,
  build-tools 35.0.0 and `javac --release 8`.
- `check-device` requires an already booted `Rustday_Edit_X64` x86_64 AVD,
  `RUSTDAY_ANDROID_SERIAL=emulator-<port>`, a non-default private
  `ANDROID_ADB_SERVER_PORT`, matching `ADB_SERVER_SOCKET=tcp:<port>`, and
  `ADB_USB=0`, `ADB_EMU=0`, `ADB_MDNS=0`. Every adb command explicitly selects that server and
  serial. Nothing enumerates, boots, wipes or stops emulators or adb servers.
  The adb executable is selected from PATH, as in the previous gates; set
  `BOARDCRAFT_ADB` to an explicit executable when the validated private server
  needs a different platform-tools version. It is not selected from `ANDROID_HOME`.
  Preflight reads the server protocol version before each device query and refuses
  a client/server mismatch rather than letting adb replace the daemon. Explicit
  numeric loopback `-H` prevents automatic daemon startup. The only device queries
  are AVD name, boot completion and ABI: no install, launch, input or reverse mapping.
- Serialize builds across workers. Internal Cargo builds take an exclusive OS
  lease on `<CARGO_TARGET_DIR>/.boardcraft-build.lock` (or `<root>/target/` without
  that variable). Other tooling must honor the same lease; Cargo's own target
  lock alone cannot serialize workers using different target directories.

## Session compatibility

The application owns [`session_store.rs`](../../android/src/session_store.rs),
which uses checked, safe `jni` calls to Android's built-in
`AndroidKeyStore`, `KeyGenerator` and AES-GCM `Cipher` APIs. The key stays in Android
Keystore. It retains alias `boardcraft.session.aes`, a 256-bit key, a Keystore-chosen
12-byte IV, 128-bit tag, version 1, exact-origin AAD and the existing 72-byte record
in `no_backup/http-127.0.0.1-<port>`. Writes sync before same-filesystem atomic rename;
only rejection or acknowledged logout removes that origin and its pending write.
Missing, corrupt and remote-unavailable states remain distinct. JNI exceptions are
cleared and logged without session contents at the application's existing boundary.

## Checks and cutover

```text
cargo fmt --manifest-path tools/android/Cargo.toml -- --check
cargo clippy --locked --manifest-path tools/android/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path tools/android/Cargo.toml
cargo test --locked --manifest-path android/Cargo.toml
cargo clippy --locked --manifest-path android/Cargo.toml --target x86_64-linux-android --lib -- -D warnings
```

The public preparation regression command above performs fresh fetch/replay,
worktree/index/untracked-file/recipe rejection and failed-preparation cleanup in an
isolated directory. The Android host tests check the real encrypted-record file
format and recovery, not Keystore cryptography or UI behavior. Use only GPUI Kit's
Rust test support and Computer Use for UI verification; if Computer Use cannot
reach Android, leave the visual check unverified. Retired UI journeys have no CLI
replacement. Actual checks and outstanding runtime coverage are recorded in
`handoffs/android-rust-tools.json`.
