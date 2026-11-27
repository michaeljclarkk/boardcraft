# Current gaps

Updated 2026-11-28. This is the short list of what is not done or not verified. A gap leaves this file when the running app shows it fixed, not when a test passes.

## UI parity

- [Live Chrome checks](workflows.md) covered workspace search, sidebar collapse/reopen, rename persistence and offline retry. The collapsed-rail temporary hover flyout (`hover_navigation`, 140ms enter / 220ms leave, `(hover: hover)`, Escape dismiss, pin toggle, no table shift, board/URL/search retained) is implemented in the WASM shell and passes WASM Clippy + native tests, but **live mouse + keyboard verification on the disposable viewer was not run in this session** (Chrome DevTools MCP / Computer Use unavailable), so it stays unverified. Remaining differences: no 300ms animated transition / `prefers-reduced-motion`; no `aria-controls`; no document-level `pointerover` rescan + menu/dialog exclusions; no compact/overlay mode. Workspace scoping, configured columns, view toolbar, concurrent-edit and same-fixture visual comparisons remain open.

- The desktop board journey has not been compared with the internal tool yet. Record every difference — layout, controls, labels, density, states, keyboard/focus, pointer/touch — in [workflows.md](workflows.md).
- Stock GPUI Kit coverage of the internal tool's table, group menu and drag behaviour is unmeasured. Build a control only when the screen needs it.

## Workflow parity

- Groups, items, board chrome, archive/trash and recovery are partial or unverified in the desktop app.
- Auth, Android and the browser are deliberately behind the desktop journey.

## Verification

- Computer Use drives Chrome only. It refuses Firefox with *"browser URL policy enforcement is not yet supported"*, so Firefox checks stay manual.
- GPUI windows expose a weak accessibility tree; some controls must be verified by screenshot and pointer position rather than element names.
- There is no custom browser driver, OCR, accessibility mirror or DOM mirror, and none will be added.

## Delivery

- No signed Windows installer or Android build is part of the current target. The app runs from `cargo`.
- The local database helper assumes the disposable `rustday_test` cluster for tests; ordinary runs use the development database.
