# Current gaps

Updated 2026-11-28. This is the short list of what is not done or not verified. A gap leaves this file when the running app shows it fixed, not when a test passes.

## UI parity

- [Live Chrome and native checks](workflows.md) cover workspace search, compact drawer, collapse/reopen and browser toolbar creation/search/reload. The 18-test web library suite passes, including eleven navigation tests and seven table tests; native sidebar controls and authorized selection/revocation also pass focused checks. Windows/WASM/Android now consume shared board header, tab, toolbar and row components. The Android debug APK was observed in the emulator: offline Sample board search and clear worked. Connected mobile saves/recovery, Back/resume and keyboard dismissal remain unverified. Final Windows toolbar focus/panel/recovery checks passed; the pre-existing default-stack overflow still requires the documented 8MB debug link workaround. Still open: same-fixture comparison, reference theme/blue accent, configured browser columns, workspace picker, native compact navigation, Android grouped data and sidebar, search timing/filtered selection parity, touchscreen journeys, `aria-controls`, menu/dialog hover exclusions and animated pinned-width changes. The existing viewer launches successfully with the documented local tool-home configuration; no launcher code was changed.

- The desktop board journey has not been compared with the internal tool yet. Record every difference — layout, controls, labels, density, states, keyboard/focus, pointer/touch — in [workflows.md](workflows.md).
- Mobile reference comparison: run the internal tool's native mobile app in a separate emulator beside boardcraft, using the same fixture and display size. Compare native controls, touch, keyboard, Back/resume and editing through Computer Use. The owner requested this as follow-up; this pass only observed boardcraft's emulator, not the internal tool on mobile.
- Stock GPUI Kit coverage of the internal tool's table, group menu and drag behaviour is unmeasured. Build a control only when the screen needs it.

## Workflow parity

- Groups, items, board chrome, archive/trash and recovery are partial or unverified in the desktop app.
- Auth remains behind the board journey. The owner explicitly authorized shared UI work on Android and WASM alongside Windows; platform behavior still differs where the port is incomplete.

## Verification

- Computer Use drives Chrome only. It refuses Firefox with *"browser URL policy enforcement is not yet supported"*, so Firefox checks stay manual.
- GPUI windows expose a weak accessibility tree; some controls must be verified by screenshot and pointer position rather than element names.
- There is no custom browser driver, OCR, accessibility mirror or DOM mirror, and none will be added.

## Delivery

- No signed Windows installer or Android build is part of the current target. The app runs from `cargo`.
- The local database helper assumes the disposable `rustday_test` cluster for tests; ordinary runs use the development database.
