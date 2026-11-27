# 57-delete-js-preview

Completely remove the JavaScript browser preview after packets 54–56 pass. The end state is one browser client: the GPUI wasm shell in `crates/web`. There is no fallback route, feature flag, redirect, parallel page or temporary second client after this packet.

**Remove every part of the preview.**

- **Assets:** `crates/mobile-edit-host/src/preview/` — all JavaScript, `index.html` and `page.css`.
- **Router:** `crates/mobile-edit-host/src/preview.rs` and the `mod preview;` declaration.
- **Host wiring:** `Mode::Preview`, `Host::start_preview`, the preview asset branch, the `/preview` and `/preview/api` route branches, and every preview-specific conditional in `crates/mobile-edit-host/src/lib.rs`.
- **Naming:** rename the `Boardcraft-Preview` header and binding to the web client's name; remove any `preview` identifier that describes the deleted client.
- **Tests:** `crates/mobile-edit-host/tests/browser_preview.rs`, the DOM harness `crates/mobile-edit-host/tests/browser/*.mjs`, and preview-only Rust checkpoint modules that nothing else uses. Delete these; do not port their assertions.
- **Build:** the `browser-smoke` feature, the `browser_preview` test target and the `browser_board_preview` example in `crates/mobile-edit-host/Cargo.toml`.
- **Docs:** every runbook command, architecture row and register link that points at the preview.

**Keep and migrate.**

- Keep the host session table, the HttpOnly cookie and exact-origin binding mechanics, the access policy and the board HTTP operations. The wasm client uses them; they are not preview code.
- Migrate host-contract tests that currently use `Host::start_preview` or `/preview/api` to the wasm client's route. Rename `preview_*` tests to the client or operation they cover; do not delete their coverage.
- Keep the minimal Trunk bootstrap page and `wasm-bindgen` glue for `crates/web`; neither contains product logic.

**Blocked, not dual.** If a workflow cannot be verified on the wasm client, the packet stays blocked and the migration stays incomplete. Do not keep the preview as a permanent second client, do not hide it behind a flag and do not redirect the old route to keep it alive. The preview may remain only while the packet is blocked; it is never an accepted end state.

**Runner gate.** The `browser_preview` gate in `scripts/run.mjs` and its cases in `scripts/run.test.mjs` are coordinator-owned. They must be retired or rewired in the same milestone. A worker packet must not edit the runner.

**Required gates** (planned, not passing evidence):

- `preview_is_completely_removed`: repository search finds no `Mode::Preview`, `start_preview`, `browser-smoke`, `browser_preview`, `browser_board_preview`, `/preview` route, `src/preview` asset or DOM harness. A remaining `preview_*` name is a migrated test with new semantics, not the old client.
- `no_hand_written_browser_app_javascript`: no product JavaScript exists outside the generated wasm bootstrap and `wasm-bindgen` glue.
- `web_browser_suite_passes_after_deletion`: the full wasm browser suite runs against the host with independent reads.
- `workspace_and_native_suites_unchanged`: locked workspace checks, desktop and Android suites still pass.
