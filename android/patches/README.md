# Android mobile host compatibility

Production Android uses Kit `=0.11.0` and the upstream mobile host at
[`7548ebc4b24f00494af9976af8852c7163ded9ce`](https://github.com/longbridge/gpui-mobile/tree/7548ebc4b24f00494af9976af8852c7163ded9ce).
`gpui-mobile.json` records that source, the diff checksum and the complete
patched Git tree. The diff is checked out with LF endings on every platform.
`gpui-mobile-kit-011.patch` is the only local upstream change:

- Align the core and renderer pins with Kit's `0.3.14` baseline. Cargo resolves
  all target constraints, so the iOS and development pins must also agree;
  this is not an iOS implementation or verification claim.
- Pass the existing Android renderer's surface configuration to the updated
  `WgpuContext::new` API. Backend selection and rendering policy are unchanged.

The patch is maintained by boardcraft. It changes no upstream author credit,
copyright notice, license file or license expression. In particular, the host's
expression remains `GPL-3.0-or-later OR AGPL-3.0-or-later OR Apache-2.0`.
No license alternative is selected here. All upstream files, including notices
and the Java `GpuiInputActivity` IME proxy, remain in the prepared checkout.
The upstream vendored renderer is not patched into the application: the
workspace uses the locked registry renderer.

## Reproduce from a public checkout

With Git and the documented Android/Rust tools available, run:

```powershell
cargo run --locked --manifest-path tools/android/Cargo.toml -- prepare-mobile
cargo tree --locked --manifest-path android/Cargo.toml --target x86_64-linux-android
cargo build --locked --manifest-path android/Cargo.toml --target x86_64-linux-android --lib
cargo run --locked --manifest-path tools/android/Cargo.toml -- test-prepare-mobile
```

Preparation fetches only the exact public revision into `android/.deps/`,
applies the tracked diff, and checks the entire resulting tree and worktree.
It is idempotent, needs network only on first preparation, refuses altered
prepared sources, and never edits Cargo's shared Git or registry sources.
`cargo run --locked --manifest-path tools/android/Cargo.toml -- build-debug --output-apk <new-absolute-path>`
performs preparation automatically and packages a signed debug APK without device
actions. Direct Cargo commands need preparation first because Cargo reads path
patches before building. UI checks use only GPUI Kit tests and Computer Use, not
the retired script or Rust UI drivers.

The Android lockfile records a path-patched host; the original Git revision
remains pinned in `android/Cargo.toml` and in the preparation provenance. No
private source, credential, screenshot, APK or signing key is a build input.
