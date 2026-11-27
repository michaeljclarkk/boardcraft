---
id: history
name: History and activity
order: 9
percent: 5
code: crates/board/src/history
---
# History and activity

Activity log, updates, revisions and undo. History is append-only and survives restart.

## Workflows

- [ ] Record an edit with actor, time and revision
- [ ] Show board and item activity
- [ ] Undo the last change
- [ ] Keep revisions when a save fails or is retried

## Notes

Lifecycle state exists; no history log is ported.
