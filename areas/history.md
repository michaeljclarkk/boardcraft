---
id: history
name: History and activity
order: 9
percent: 10
code: crates/board/src/history, crates/mobile-edit-host/src/activity.rs, crates/mobile-edit-client/src/activity.rs, crates/desktop/src/connected/main_table/activity.rs, android/src/navigation/activity.rs
---
# History and activity

Activity log, updates, revisions and undo. History is append-only and survives restart.

## Workflows

- [x] Record an item rename with actor, time and revision
- [ ] Show board and item activity
- [ ] Undo the last change
- [ ] Keep revisions when a save fails or is retried

## Notes

Windows item activity was observed on Review. Device UI, board-wide activity, other fields and undo remain open.
