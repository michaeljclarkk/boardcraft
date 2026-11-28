# Current gaps

Updated 2026-11-28. This is the short list of what is not done or not verified. A gap leaves this file when the running app shows it fixed, not when a test passes.

## UI parity

- [Live Chrome checks](workflows.md) now cover the rewritten workspace search and collapsed-rail hover flyout as well as the earlier rename/retry journey. The baseline's duplicate-`on_hover` panic is fixed in the production Rust [`Navigation`](../crates/web/src/navigation.rs) view; pointer entry, movement into the panel, leave dismissal and no table shift were observed. Twelve web tests (including six GPUI navigation interaction tests), WASM build and WASM/native Clippy passed. Search is scoped to the selected board's workspace and resets on workspace change (GPUI-tested, not live multi-workspace verified); the landing page falls back to the first authorized workspace because the workspace picker is not ported. Still open: same-fixture visual comparison, 300ms slide/reduced-motion handling, `aria-controls`, menu/dialog hover exclusions and document-level hover rescan, compact drawer/touch, configured columns, view toolbar and active/concurrent-edit comparison. A later bounded-viewer relaunch returned `program not found` after concurrent tools changes; the preceding after-change run succeeded and expired normally. No launcher changes were made by this rewrite.

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
