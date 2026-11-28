---
id: ui
name: UI components
order: 12
percent: 35
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

Windows, WASM and Android now use `boardcraft-ui`'s `BoardHeader`, `BoardTabs`, `BoardToolbar` and `BoardRow` for the board title, view tabs, action layout and column geometry. Transport, drafts and the remaining screen composition still belong to each client. Android requests the shared touch density. This is shared screen code, not complete screen or workflow parity; search timing, grouped mobile data, themes and unported controls still differ. The estimate increased from 30% to 35% on 2026-11-28 for observed compact navigation, retained native workspace search, working board toolbar actions and shared controls rendered on all three clients, including Android sample-board filtering. This is partial credit for verified behavior, not full visual parity or connected Android editing. Full same-fixture comparison remains open.
