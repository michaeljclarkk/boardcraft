---
id: notifications
name: Notifications
order: 8
percent: 10
code: crates/board/src/notifications.rs, crates/mobile-edit-host/src/notifications.rs, crates/mobile-edit-client/src/notifications.rs, crates/desktop/src/connected/navigation.rs, android/src/navigation.rs
---
# Notifications

Inbox, preferences, seen state and email/push delivery. A notification points at the record and action that produced it.

## Workflows

- [ ] Receive an assignment and an owned-item update
- [x] Read and mark seen
- [ ] Configure per-category preferences
- [ ] Open the exact item from the notification
- [ ] Deliver by email when configured

## Notes

The first slice stores a recipient-scoped inbox row when an enabled automation records a run, in the same transaction as the rename. Migration 16 adds `inbox_notifications`: board, item, automation, creator as recipient, title/message, created/read/seen/deleted timestamps. The host lists only the resolved session's rows and supports read/unread, read-all, seen and delete; the shared client exposes the same calls.

Windows browse opened `Notifications (1)`, showed `1 unread`, marked rows seen on load, toggled read/unread, marked all read, deleted a row, and `Open` marked the row read, opened the board and focused the exact item (`Ink 3`). SQL confirmed one receipt per run, the read/seen timestamps and soft deletion. The first run exposed a duplicate accessibility node id while drawing several inbox rows; every row now has its own container and label ids and the rerun stayed up. Android compiles for `x86_64-linux-android` and renders the same inbox, but its device screen was not observed and `Open` only reaches the board, not the item.

`Reload board` does not refresh the unread count because it preserves the loaded table; reopening the board or opening the inbox does. No browser UI exists. There is no new `crates/notifications` crate: the code lives in the existing board/host/client/desktop/Android homes, so no crate or dependency was added. Assignment and owned-item notifications, preferences, read-state undo, pagination beyond 200 rows, email/push delivery and a run-linked notification card remain open.
