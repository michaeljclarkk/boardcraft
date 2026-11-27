# 53-web-shell

Build the browser client as a Rust GPUI WebAssembly application and serve it from the existing loopback host. This packet delivers the shell and session binding only; board workflows follow in 54 and 55.

**Stack.** Add `crates/web` (package `boardcraft-web`) to the workspace with `gpui-kit = "=0.11.0"` and `wasm-bindgen`. Build for `wasm32-unknown-unknown`. Keep `crate-type = ["cdylib", "rlib"]`. Do not depend on `crates/desktop`, the `windows` crate, native windowing or the blocking `postgres` client. See [the decision](../docs/web-client-stack.md).

**Shell.** One GPUI window and one canvas. Embed every font and asset; the browser has no system fonts and no filesystem. Register `boardcraft-ui` controls through `gpui_kit::init`. No native-only Kit layer. If a required control is missing on wasm, stop and record it rather than adding a web-only widget.

**Missing controls.** Extend `crates/boardcraft-ui` instead of routing around GPUI. A control added or fixed here needs a public API, a synthetic example, an interaction test, documented keyboard/focus/accessibility behaviour and a note on the missing Kit capability. Do not embed HTML, CSS, JavaScript, React/TSX, a webview, an iframe or a DOM overlay, and do not keep a web-only control in `crates/web`. Record the control in [the register](../docs/gpui-components.md).

**Page.** Trunk builds the development page. The host serves the built artifacts on the preview origin with `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`. Only the minimal Trunk bootstrap page and generated `wasm-bindgen` glue may exist as HTML/JavaScript; neither contains product logic. No hand-written application JavaScript. Keep the existing `/preview/` JavaScript route working until packet 57.

**Session.** The host keeps the bearer. The wasm client sends same-origin requests and relies on the host-only HttpOnly cookie plus the exact-origin binding. The browser never receives the bearer. Reuse the existing session table and handlers; do not add a second authentication mechanism or a token in JavaScript memory.

**Required gates** (planned, not passing evidence):

- `web_shell_loads_and_resolves_session`: launch an owned headless Chromium against the host, confirm the canvas mounts, a real session resolves and the signed-in shell appears. Assert no direct PostgreSQL connection is opened from the wasm process.
- `web_shell_handles_unavailable_and_rejected_session`: an unavailable resolve offers retry on the same session; a rejected session returns to the unauthenticated shell. No stale protected content survives.
- `web_shell_serves_cross_origin_isolation`: the served response carries the required COOP/COEP headers and the module instantiates under a secure origin.

Record bundle size and first-load time in the handoff. Inherit the locked workspace checks. Worker owns the crate, host route and evidence; the coordinator registers the runner gate and verifies the packet.
