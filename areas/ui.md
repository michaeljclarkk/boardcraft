---
id: ui
name: UI components
order: 12
percent: 42
code: crates/boardcraft-ui, crates/desktop/src, crates/web/src, android/src
---
# UI components

GPUI Kit controls, layout, theme, dialogs, keyboard and focus. Shared controls live in `boardcraft-ui`; screens own their composition and drafts.

## Workflows

- [x] Board list, main table and group headers
- [x] Inline item rename with focus and cancel
- [x] Dialogs for group fields and selection move
- [ ] Full theme, density and typography parity
- [ ] Menus, tooltips and keyboard shortcuts
- [ ] Empty, loading and error states everywhere

## Notes

Windows, WASM and Android use shared Rust board chrome. Windows/WASM now compose `BoardSelectionBar` below the scrolling table, with a count tile, Archive, Delete, Move to and Clear, restored dialog focus and embedded action icons. Android uses `BoardFindDock` and a Rust native filter dialog with status/group choices, delayed preview, Apply/Close retention and Discard restoration. Its grouped touch rows expose name editing, Status and activity within the phone width. Android uses visible text indicators where its host has no SVG assets. No second view layer was added.

The area estimate increased from 38% to 42% on 2026-11-28 for observed bottom selection actions/focus in Windows and Chrome, connected Android groups/collapse/search/hidden selection/Home, filter Apply/Discard/Close/reload, and blank-name correction/retry. Search accepts the latest query after 250ms on Windows/WASM and the native dock's combined 350ms on Android; overlong UTF-16 input retains prior results. Filter preview uses the native source's 200ms debounce. Exact timings are code/test evidence, not screenshot measurements. See [observations](../docs/workflows.md).

Native Main table estimates: **Windows 44%; Android 38%**, up from 40% and 30%. These are conservative screen-level port estimates, not measured percentages of the entire native applications or all MAUI components. Workflow, UI and UX parity are all required. Full selection actions, theme/density, configured columns, native footer/navigation, persisted/advanced filters, item/update composer, Android focus-loss save, conflict/offline recovery and Back/resume remain open. The separate Tuesday native mobile-app comparison has not run. These native estimates are not averaged to calculate the area percentage.

Reference Chrome, final Rust Chrome and final Rust Windows used 681×1025 outer windows, with the Windows sidebar manually collapsed. Disposable Rust data matched Software Development / boardcraft test / New group / Sketch / Ink, with native Ink Stuck; Android's rename check then saved `ink`, also visible in Windows. The clients used different data identities, browser Status remains absent, and browser decoration changes the content height: full same-fixture and pixel parity are not claimed. Source review confirms the MAUI mobile board is a horizontally scrolling table with a capped frozen name column; the current Rust phone layout is not yet that geometry.
