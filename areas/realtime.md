---
id: realtime
name: Realtime
order: 10
percent: 10
code: crates/mobile-edit-host/src/realtime.rs, crates/mobile-edit-client/src/realtime.rs, crates/desktop/src/connected/navigation.rs, android/src/lib.rs, android/src/realtime.rs
---
# Realtime

Live board refresh and presence: the SignalR equivalent. A second client sees an accepted change without a manual reload.

## Workflows

- [x] Push an accepted board change to another client
- [x] Refresh only the affected rows
- [ ] Reconnect and catch up after a disconnect
- [ ] Drop updates for a session that lost access

## Notes

The host keeps one bounded change history and serves it as a bearer-authorized SSE feed (`GET /navigation/events`). A committed rename publishes `{seq, board_id, item_id, kind:"item_renamed"}` only after the transaction commits; no rolled-back change can reach a subscriber. A connecting client receives the retained history, so a change made between its board read and its connect is not lost. `GET /navigation/events/items/{board}/{item}` reads the exact accepted row through the normal board authorization; the feed itself carries no title or content.

The client exposes a blocking `ChangeFeed::next()` plus `read_main_table_item`. Windows owns one feed per browse window across board changes, filters by the open board, reads the named row off the UI thread, and replaces only that row's accepted title/revision. A pending save or a failed row keeps its local path; a clean row applies only a newer revision. The two-window check on the third Review fixture showed `Ink` become `Ink live` in the second window without `Reload board`, while `Sketch live`, focus and every other row stayed put.

tiny_http buffers chunked output in 8 KiB blocks, so each event is followed by a padding comment to flush the data line; an idle feed sends a padded keep-alive every 10 seconds. Android builds the same feed/pump into its board view and linked for `x86_64-linux-android` with Clippy `-D warnings`, but was not observed on a device. The desktop direct-board `connect` window is not subscribed.

Not done: automatic reconnect and catch-up (a dropped feed stays stopped), backlog coalescing, dropping a subscription after access is revoked, a browser subscription, presence, and closing the feed's worker promptly when a window closes (it exits at its next event). No new crate was added; the code lives in the existing host, client, desktop and Android homes.
