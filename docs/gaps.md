# Current gaps

Updated 2026-11-28. This is the short list of what is not done or not verified. A gap leaves this file when the running app shows it fixed, not when a test passes.

## UI parity

- Status Edit labels is implemented across Rust clients with per-column palettes, Owner permissions, configured default name/color, autosave, New label and saved-draft recovery. Windows now has live name/default-color/new-label, restart, rejected-draft recovery, explicit Retry and Escape/focus evidence; browser read those Windows saves on the same board and retained selection/Add item draft across editor dismissal. Android emulator5580 was reopened, labels-check.apk updated without resetting data, and test sign-in succeeded; connected table observation is still in progress. The original native Android app was reopened and its normal Change server recovery reaches the live service's login page, so signed-in native reference comparison remains blocked. Final browser Custom #123456 save, current-color swatch and equal odd tracks were observed. Custom uses Kit rather than the browser OS dialog and closes the parent palette after save, unlike the source. Exact native dimensions and full touch/lifecycle parity remain open. Source inspection confirms configured default and Clear both store null; earlier entries describing separate non-null Empty semantics were incorrect.

- Bulk Change label for existing Status is implemented across clients and verified live in Windows and the integrated browser, including retained selection, partial success, explicit retry, Clear and cross-client reload. The Windows two-conflict accessibility crash and browser chooser height were fixed and rerun. Android's installed `bulk-check.apk` still opens at sign-in; connected bulk touch/focus/lifecycle is unverified. Label-management native verification, summaries, failure presentation and exact native dimensions remain gaps. Columns 15%; no Android credit. See the latest workflow entry for checks and isolated-build limits.

- Views now persist title/Status/Blank/physical-group presets. Windows save/restart/apply and Blank/Discard were observed; browser preview/Escape was observed, while browser save/reopen is blocked by a clipboard permission prompt pending user dismissal. Android's updated companion still needs connected sign-in. Shared editing, active/default view persistence, sort/derived grouping, advanced filters, Item chips, draft-review/save-to-current-view and source popover placement remain missing. Views 13%. Concurrent automation changes currently remove legacy client APIs and the host endpoint that Windows loads when opening a board; the Views-only checkout passes and runs, but combined integration needs the automation team's compatible consumers.

- Status now displays and edits in the Rust browser. Windows/browser clear, selected/empty focus, cross-client reads and live stale-save recovery were observed; see the latest workflow entry. Android's latest companion is `bulk-check.apk`, but connected Status remains unverified. Native label-management verification, summaries, exact source widths/styles and aria-selected metadata remain open. Columns 15%.

- Latest continuation: Night palette, browser floating selection and compact Windows header/footer were observed. Windows/browser shared a disposable board identity; reference content matched but reference IDs differed. Windows resizing through Computer Use did not succeed (1538×1127 versus 681×1025 Chrome), so live compact-drawer parity stays open despite GPUI coverage. Android frozen-name table is implemented, built and installed in the preserved companion package; its prior host expired and new test sign-in could not be completed through Computer Use. Do not treat the older foreground app's swipe as verification of the updated companion. UI 43%; native Main table Windows 45%, Android 38%.

- [Current Main table observations](workflows.md) now include Windows/browser bottom selection bars, visible action icons and dialog focus restoration; connected Android groups, collapse, hidden selection, Clear/Cancel/Home, name validation/correction/retry, and filter Apply/Discard/Close/reload. Seventeen native, twenty-one web and five shared tests passed; WASM build, focused Clippy, Android target Clippy/package and formatting passed. Android focus-loss save, graphical conflict/offline recovery and system Back/resume remain unverified. Default-stack Windows rendering still overflows; runtime verification used the existing 8MB debug link workaround, not a fix.

- Earlier reference Chrome, Rust Chrome and Rust Windows were compared at 681×1025 outer dimensions, with Windows navigation manually collapsed. Disposable data reproduced Software Development / boardcraft test / New group / Sketch / Ink; the Android retry check then saved `ink`. Identities differ and browser Status was absent in that earlier comparison. This is matched visible content, not full shared-fixture parity. Typography, row density, global header, selection-bar dimensions/full actions, configured columns, desktop Person/Filter/Hide/Group by and workspace picker remain different. Automatic native compact navigation is implemented and GPUI-tested but not observed live. Search still matches names only, without source tags/paging.
- Native MAUI components remain in scope. Android's visible Filter now opens the Rust dialog, but persisted view settings, advanced expressions, other column tabs and alternative grouping remain missing. Item/update mode lacks an existing composer transport and is not ported. Source review confirms a horizontally scrolling mobile table with a viewport-capped frozen name column, not cards; the new Rust frozen-name geometry remains unverified. Collapse is local, source icons/native Home/workspace presentation remain incomplete, and the current host has no SVG assets. Do not count compilation as mobile visual parity. UI estimate is 43%; native Main table estimates are Windows 45%, Android 38%; no whole-native-app percentage has been measured.
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

## Automation

- The rule engine, migration, host routes, Windows panel/wizard and Android screen are implemented. PostgreSQL engine/replay/migration tests and the owner/member HTTP route test pass; Windows compiles and its panel/tab GPUI control test passes; Android compiles for `x86_64-linux-android`.
- Browser Manage/Run history/wizard UI is not rendered or integrated. `crates/web/src/automation.rs` currently holds only a partial state/draft model.
- No live signed-in Windows or Android automation screen was observed. Button-click, conversation/mention/reaction triggers, people/date actions and dependency tracking are not ported because those board features are not ported yet.
