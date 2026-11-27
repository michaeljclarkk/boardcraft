# Android native shell

Run from the checkout root. Android is a separate locked Rust workspace. The app
owns its Kit views, worker-driven HTTP session resolution and safe JNI Keystore
storage; it does not depend on the operator-tool crate. The original version-1
AES-GCM record, exact loopback-origin AAD, Keystore alias, atomic writes and
rejection/logout cleanup remain unchanged. The only compiled Java is the pinned
upstream transparent IME activity, not an app-authored session helper.

## Prepare and package

Use Git, Rust with `x86_64-linux-android`, JDK 21 (including `javac --release 8`),
Android SDK platform 35/build-tools 35.0.0 and an installed NDK with API 28 x86_64
compilers. Configure `ANDROID_HOME`, `JAVA_HOME` and these process variables:

- `CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER` and `CC_x86_64_linux_android`: the NDK's
  `toolchains/llvm/prebuilt/windows-x86_64/bin/x86_64-linux-android28-clang.cmd`.
- `CXX_x86_64_linux_android`: the adjacent `x86_64-linux-android28-clang++.cmd`.
- `AR_x86_64_linux_android`: the adjacent `llvm-ar.exe`.

```text
cargo run --locked --manifest-path tools/android/Cargo.toml -- prepare-mobile
cargo run --locked --manifest-path tools/android/Cargo.toml -- test-prepare-mobile
cargo build --locked --manifest-path android/Cargo.toml --target x86_64-linux-android --lib
cargo run --locked --manifest-path tools/android/Cargo.toml -- build-debug --output-apk <new-absolute-path>
```

The [preparation recipe](patches/README.md) validates the complete pinned source
and rejects tampering. Packaging creates and removes its own disposable signing
key, verifies the signed APK and never overwrites an output or contacts a device.
There are no PowerShell helper entry points. General configured child commands use
`cargo run --locked --manifest-path tools/Cargo.toml -- dev -- <program> <args>`;
database children use `cargo run --locked --manifest-path tools/Cargo.toml -- postgres exec -- <program> <args>`.

## Verify

```text
cargo fmt --manifest-path android/Cargo.toml -- --check
cargo test --locked --manifest-path android/Cargo.toml
cargo clippy --locked --manifest-path android/Cargo.toml --all-targets -- -D warnings
cargo clippy --locked --manifest-path android/Cargo.toml --target x86_64-linux-android --lib -- -D warnings
cargo run --locked --manifest-path tools/android/Cargo.toml -- check-device
```

Host tests exercise the production encrypted-record file code without compiling
the Android-only shell. They do not prove Android Keystore runtime compatibility.
The read-only device command's dedicated-emulator prerequisites are documented in
[operator tools](../tools/android/README.md); it does not install or drive the app.

UI verification uses GPUI Kit's Rust test support and Computer Use only. If
Computer Use cannot reach the target, record the UI check as unverified. Do not
replace the retired frame/edit/reopen/OCR journeys with another UI harness.
The no-argument app still consumes the synthetic offline board in
`scripts/fixtures/android-kit-board.json`; connected launches keep their existing
board/port and browse semantics. No lifecycle or session behavior is replaced by
packaging or host-only tests.
