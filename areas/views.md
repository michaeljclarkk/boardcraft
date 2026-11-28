---
id: views
name: Views and charts
order: 6
percent: 10
code: crates/board/src/views, crates/web/src/views
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

The first slice saves a named item-title filter as a personal view in PostgreSQL, lists shared and personal views when a board opens, and applies the filter to the main table. Windows browse verified Apply filter, Clear filter (Enter and the buttons), Save view, reopening the board, reopening the app and applying the saved view again. Android lists and applies views; its display was not observed on a device. Browser UI, sorting/grouping, chart forms, sharing a view and UI parity with the reference remain open. In this session the debug desktop build overflowed the default 1 MB main-thread stack while drawing any board, including unmodified HEAD code; with an 8 MB link stack it rendered normally. That pre-existing build issue is not fixed here.
