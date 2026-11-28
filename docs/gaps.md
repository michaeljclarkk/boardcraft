# Current gaps

Updated 2026-11-28. This is the short list of what is not done or not verified. A gap leaves this file when the running app shows it fixed, not when a test passes.

## UI parity

- [Current Main table observations](workflows.md) now include Windows/browser bottom selection bars, visible action icons and dialog focus restoration; connected Android groups, collapse, hidden selection, Clear/Cancel/Home, name validation/correction/retry, and filter Apply/Discard/Close/reload. Seventeen native, twenty-one web and five shared tests passed; WASM build, focused Clippy, Android target Clippy/package and formatting passed. Android focus-loss save, graphical conflict/offline recovery and system Back/resume remain unverified. Default-stack Windows rendering still overflows; runtime verification used the existing 8MB debug link workaround, not a fix.

- Reference Chrome, final Rust Chrome and final Rust Windows were compared at 681×1025 outer dimensions, with Windows navigation manually collapsed. Disposable data reproduced Software Development / boardcraft test / New group / Sketch / Ink; the Android retry check then saved `ink`. Identities differ and the browser lacks Status. This is matched visible content, not full shared-fixture parity. Theme, typography, blue accent, row density, header spacing, selection-bar dimensions/full actions, native footer controls, configured columns, desktop Person/Filter/Hide/Group by, workspace picker and automatic native compact navigation remain different. Browser selection reserves bottom space rather than fully matching the reference overlay. Search still matches names only, without source tags/paging.
- Native MAUI components remain in scope. Android's visible Filter now opens the Rust dialog, but persisted view settings, advanced expressions, other column tabs and alternative grouping remain missing. Item/update mode lacks an existing composer transport and is not ported. Source review confirms a horizontally scrolling mobile table with a viewport-capped frozen name column, not cards; the current compact Rust row geometry is still different. Collapse is local, source icons/native Home/workspace presentation remain incomplete, and the current host has no SVG assets. Do not count compilation as mobile visual parity. UI estimate is 42%; native Main table estimates are Windows 44%, Android 38%; no whole-native-app percentage has been measured.
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
