# 57-delete-js-preview

Keep one browser client: the GPUI wasm shell in `crates/web`. Removal of the JavaScript preview from the checkout does not verify the replacement workflows in packets 53–56.

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

**Acceptance.** Check the active source and build wiring for removal; leave historical packet IDs and handoffs alone. Keep the locked workspace, GPUI Kit and non-UI host/store checks. Launch the [normal viewer](../docs/development.md#browser-viewer), try the `/web/` client with Computer Use and record the actual visible outcomes, including after rebuilding and restarting the host. Keep captures local and credential-free. Without observations for the required journeys, leave acceptance pending. Do not restore a driver or the preview to close the packet.
