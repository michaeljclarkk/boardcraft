---
name: boardcraft-feature
description: Implement or review a boardcraft workflow across Rust, PostgreSQL and GPUI, including state ownership, failure recovery and milestone evidence.
---

Start with a user action. What record does it affect, what must stay true, and what will the user see if saving fails?

- **One client stack.** Desktop, Android and the browser use GPUI Kit. Browser work targets the wasm shell in `crates/web` and shared controls in `crates/boardcraft-ui`; both must compile for `wasm32-unknown-unknown`. Do not add hand-written browser application JavaScript, keep the JavaScript preview as a fallback, or add a second view layer. See [the decision](../../../docs/web-client-stack.md).
- **No UI fallback.** If Kit lacks an interaction, build it in `crates/boardcraft-ui` with a public API, docs, a synthetic example, an interaction test and accessibility semantics. Do not embed HTML/CSS/JS/React/TSX, a webview, iframe or DOM overlay, and do not keep a web-only control. Keep it domain-free and document the missing Kit capability so it can be shared or proposed upstream later; an upstream PR is not required in a packet. See [shared GPUI components](../../../docs/gpui-components.md).
- **One rule owner.** Trace the interaction from the Kit control to a named Rust operation and back. A keyboard shortcut, context menu and import performing the same action should call that operation. Use stable record IDs; titles and row positions can change.
- **One commit point.** Keep related PostgreSQL writes in a transaction. Report the accepted value after commit. If an edit also schedules required work, persist that intent with the edit and make retries safe. Only add this machinery to a feature that needs it.
- **A recoverable editor.** Run I/O away from the GPUI event thread. Distinguish an editable draft from a pending request and a saved value. On failure, keep the draft and give the user a way to retry or reload. Show a stale edit as a conflict rather than silently overwriting another user's change.
- **Proportionate reads.** Fetch the rows and fields the view needs in a stable order. Check for a query per row before reaching for a cache. Prefer a stock Kit control unless a specific interaction requires a custom one.

Test at the boundary where a mistake would show up. For a rename, create two items with the same title, edit one, reconnect, and check both IDs and their order. Interrupt a save and verify the stored value and the draft independently. In the handoff, identify the rule owner, the check that passed, and anything that still needs to be exercised in the running app.
