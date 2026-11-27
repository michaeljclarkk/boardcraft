# 53-web-shell

Build the browser client as a Rust GPUI WebAssembly application and serve it from the existing loopback host. This packet delivers the shell and session binding only; board workflows follow in 54 and 55.

**Stack.** Add `crates/web` (package `boardcraft-web`) to the workspace with `gpui-kit = "=0.11.0"` and `wasm-bindgen`. Build for `wasm32-unknown-unknown`. Keep `crate-type = ["cdylib", "rlib"]`. Do not depend on `crates/desktop`, the `windows` crate, native windowing or the blocking `postgres` client. See [the decision](../docs/web-client-stack.md).

**Shell.** One GPUI window and one canvas. Embed every font and asset; the browser has no system fonts and no filesystem. Register `boardcraft-ui` controls through `gpui_kit::init`. No native-only Kit layer.

**Missing controls.** If Kit lacks a required interaction, build or extend a reusable Rust control in `crates/boardcraft-ui`. Follow the [shared component process](../docs/gpui-components.md): document the public API, the Kit gap and keyboard, focus and accessibility behaviour; add a runnable synthetic example, an interaction test and a register entry. Build and exercise the control on both desktop and wasm, and record the results. Report a blocker only if the shared implementation cannot support the required interaction. Do not embed HTML, CSS, JavaScript, React/TSX, a webview, an iframe or a DOM overlay, and do not keep a web-only control in `crates/web`.

**Page.** Trunk builds the development page. The loopback host serves `/web/` with `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`. Only the minimal Trunk bootstrap page and generated `wasm-bindgen` glue may exist as HTML/JavaScript; neither contains product logic.

**Session.** The host keeps the bearer. The wasm client sends same-origin requests and relies on the host-only HttpOnly cookie plus the exact-origin binding. The browser never receives the bearer. Reuse the existing session table and handlers; do not add a second authentication mechanism or a token in JavaScript memory.

**Acceptance.** Launch the [ordinary viewer](../docs/development.md#browser-viewer). With Computer Use, observe the signed-in shell, keyboard retry after an unavailable resolve, and removal of protected content after session rejection. Keep the host header, origin-binding and store checks separate. Use GPUI Kit tests for control behavior. Record the bundle size, observed first-load time and any limits of the measurement. Without the live observations, leave the packet pending; the earlier driver handoff is historical.
