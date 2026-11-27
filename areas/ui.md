---
id: ui
name: UI components
order: 12
percent: 30
code: crates/boardcraft-ui, crates/desktop/src, crates/web/src
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

The desktop and wasm shells share controls. Many Tuesday controls are still missing.
