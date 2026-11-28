# Current gaps

Updated 2026-11-28. This is the short list of what is not done or not verified. A gap leaves this file when the running app shows it fixed, not when a test passes.

## UI parity

- [Current Main table observations](workflows.md) cover Windows/browser delayed search, matching counts, hidden selection and clear restoration, and Android offline dock filtering, visible Clear and Cancel/keyboard dismissal. Nineteen web, fourteen native and five shared-component tests pass; Android target Clippy/package and formatting pass. Connected Android groups, collapse, filtered selection and Home now compile but were not observed: the foreground attempt reached sign-in, then Computer Use timed out and reset before the connected board. Connected saves/recovery and system Back/resume remain unverified. Default-stack Windows rendering still overflows; runtime verification used the existing 8MB debug link workaround, not a fix.

- Reference Chrome and Rust Chrome were compared at the same 681×1025 outer dimensions; Rust Windows was resized to the same outer dimensions and manually collapsed. Disposable data reproduced Software Development / boardcraft test / New group / Sketch / Ink, but identities differ and the browser lacks Status. This is matched visible content, not full shared-fixture parity. Theme, typography, blue accent, row density, header spacing, top versus bottom selection actions, native footer controls, configured columns, Person/Filter/Hide/Group by, workspace picker and automatic native compact navigation remain different. Search still matches names only, without source tags/paging.
- Native MAUI components remain in scope. The Rust quick-find dock is partial: Android Clear uses visible × and pending uses … because SVG assets are absent; magnifier, filter button/editor, item/update composer and native Home/workspace presentation are not at parity. Connected Android grouping is a touch table, not a verified match for the reference mobile cards, and collapse is local to the visit. Other Android icons remain missing. Do not count compilation as mobile visual parity. Native Main table estimates are Windows 40%, Android 30%; no whole-native-app percentage has been measured.
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
