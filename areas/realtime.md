---
id: realtime
name: Realtime
order: 10
percent: 0
code: crates/mobile-edit-host/src/realtime, crates/mobile-edit-client/src/realtime
---
# Realtime

Live board refresh and presence: the SignalR equivalent. A second client sees an accepted change without a manual reload.

## Workflows

- [ ] Push an accepted board change to another client
- [ ] Refresh only the affected rows
- [ ] Reconnect and catch up after a disconnect
- [ ] Drop updates for a session that lost access

## Notes

Not started. Polling is not a substitute.
