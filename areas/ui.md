---
id: ui
name: UI components
order: 12
percent: 38
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

Windows, WASM and Android use `boardcraft-ui`'s `BoardHeader`, `BoardTabs`, `BoardToolbar` and `BoardRow`. Android also uses the shared Rust `BoardFindDock`, translated from the native MAUI quick-find dock: bottom search pill, Clear, Cancel, pending state and retained focus. Its connected Home returns to the existing board list; the native filter editor and item/update composer are still missing. Android uses visible text indicators where the current host has no icon assets. No second view layer was added.

The area estimate increased from 35% to 38% on 2026-11-28 for observed Windows/browser matching counts and retained hidden selection, plus Android dock filtering, Clear and Cancel. Search accepts the latest query after 250ms on Windows/WASM and the native dock's combined 350ms on Android; overlong UTF-16 input retains prior results. Android connected groups, counts, collapse and selection are implemented and compile, but have no new connected runtime credit. See [observations](../docs/workflows.md).

Native Main table estimates, first recorded this slice: **Windows 40%; Android 30%**. These are conservative screen-level port estimates, not measured percentages of the entire native applications or all MAUI components. Windows has observed navigation, rows, search/selection and existing commands, with substantial footer, theme, columns and compact-layout differences. Android has observed shared touch rows and the search dock; connected grouping/selection, Home, editing/recovery and Back/resume still need foreground verification. Dock filters, item/update mode, native navigation and the separate Tuesday mobile-app comparison remain open. These native estimates are not averaged to calculate the area percentage.

Reference Chrome and Rust Chrome used the same 681×1025 outer window. Rust Windows was also resized to 681×1025 with its sidebar manually collapsed. Disposable Rust data matched the reference's visible Software Development / boardcraft test / New group / Sketch / Ink content, with native Ink Stuck. The clients used different data identities, browser Status remains absent, and browser decoration changes the content height: full same-fixture and pixel parity are not claimed.
