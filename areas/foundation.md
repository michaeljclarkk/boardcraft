---
id: foundation
name: Foundation: database and store
order: 1
percent: 40
code: crates/board/src/store, crates/board/migrations
---
# Foundation: database and store

Schema, migrations, board/group/item persistence, transactions and failure recovery. Everything else depends on these rules being correct and durable.

## Workflows

- [x] Persist board, group and item records
- [x] Rename an item and read it back after reconnect
- [x] Archive and restore a root or group
- [ ] Column values, cells and their validation
- [ ] Import, history and derived data storage

## Notes

Board, group, item, order, selection and lifecycle tables exist. Column and view storage do not.
