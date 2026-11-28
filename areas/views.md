---
id: views
name: Views and charts
order: 6
percent: 13
code: crates/board/src/views.rs, crates/desktop/src/connected/main_table/views, crates/web/src/groups/views.rs, android/src/navigation/filters.rs
---
# Views and charts

Saved views, filters, sorting, grouping, chart settings and forms. Shared and personal settings stay distinct.

## Workflows

- [x] Save and reopen a view
- [ ] Filter, sort and group
- [ ] Chart settings and drill-down
- [ ] Form configuration and submission
- [ ] Share a view without granting access

## Notes

2026-11-28: personal presets now persist title search, Status/Blank and physical-group choices. Windows and browser have delayed preview and explicit draft recovery; Android saves the same contract. Windows live verification saved Stuck review, restarted, reopened and applied it; Blank replaced the label filter and Discard restored Stuck. Browser live preview/Escape retention passed, but saving/reopening is not yet graphically verified because Chrome requested clipboard permission. Thirty native and twenty-nine web GPUI tests, cross-client PostgreSQL privacy/permissions/reopen checks, WASM build and Android target Clippy/build passed. Android companion was updated without resetting data; connected verification awaits sign-in. Shared presets are readable, not editable/shareable here. Advanced expressions, sorting, derived grouping, active/default view persistence, source draft-review/save-to-view flow and exact popover presentation remain open. Views 13%; no full parity claim.

The first slice saves a named item-title filter as a personal view in PostgreSQL, lists shared and personal views when a board opens, and applies the filter to the main table. Windows browse verified Apply filter, Clear filter (Enter and the buttons), Save view, reopening the board, reopening the app and applying the saved view again. Android lists and applies views; its display was not observed on a device. Browser UI, sorting/grouping, chart forms, sharing a view and UI parity with the reference remain open. In this session the debug desktop build overflowed the default 1 MB main-thread stack while drawing any board, including unmodified HEAD code; with an 8 MB link stack it rendered normally. That pre-existing build issue is not fixed here.
