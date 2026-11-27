# Browser client stack: GPUI on WebAssembly

The browser client is the Rust/WASM application in [`crates/web`](../crates/web). The JavaScript preview and the custom browser test suites are historical. Their handoffs describe the builds that ran then; they do not verify this client.

## Owners

- **View:** one GPUI Kit window and canvas, embedded fonts and assets, ordinary text input, masked passwords and keyboard navigation. Trunk supplies the bootstrap page and generated wasm-bindgen glue; neither owns product logic.
- **Controls:** desktop, Android and web target Kit `=0.11.0`. Put missing shared interactions in [`boardcraft-ui`](../crates/boardcraft-ui), with a GPUI Kit test and a synthetic example. The shared crate must build natively and for `wasm32-unknown-unknown` without domain, database or native-window dependencies. Do not mirror controls in the DOM.
- **Host:** the loopback host serves the bundle with COOP/COEP isolation. The HttpOnly cookie and exact-origin binding select a host-owned bearer. The host checks current access; the store commits the change. Only the view owns drafts and pending edits.

## Verification

Launch the [normal viewer from the Rust CLI](development.md#browser-viewer). Use **Computer Use** to try a workflow in the ordinary browser and record the action and visible result. Use **GPUI Kit tests** for control behavior. These are the only UI verification mechanisms; do not build a browser/GUI driver, use OCR or CDP, or substitute another harness in any language.

Keep the locked wasm build and the [non-UI contract tests](../crates/mobile-edit-host/tests/loopback_board_access.rs). For example, when editing one of two identically titled rows, check the visible selection and focus with Computer Use; compare the requested UUID, unchanged peer, order and committed revision through independent host/store reads. A passing build, Kit test or store check cannot supply a missing live observation. Leave that journey pending. Visible text masking alone is not evidence of screen-reader or IME privacy.

## Migration packets

| Packet | Acceptance target |
|---|---|
| 53 | Load the shell and resolve, retry or reject a session. |
| 54 | Sign in, navigate, rename and recover; revisit a board in a second tab. |
| 55 | Exercise group, selection, ordering, Archive and Trash workflows. |
| 56 | Recheck canvas input, focus, identity and recovery against the workflow register. The historical packet name does not authorize a harness. |
| 57 | Keep one client with no preview assets, routes, feature flag or fallback. Deletion alone does not establish parity. |
