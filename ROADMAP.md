# First vertical slice: edit an item, close the app, see the edit again

These are initial acceptance targets, not claims of parity. Each packet inherits the previous one's behavior. Use Cargo at the repo root. The named integration test targets below must contain real, enabled assertions.

### 01-board

Create the Rust workspace and board model. On a synthetic board, create two uniquely identified items with the same title. Rename one by identity: its order and the other item must not change. Editing a missing item returns a recoverable outcome and leaves the board intact. Cover both paths in the `board_edit` integration test. No database or renderer needed yet.

### 02-postgres

Add migrations and persist the board operation in PostgreSQL. Test on a dedicated, disposable Rustday database supplied as `RUSTDAY_TEST_DATABASE_URL`. No fallback to other URLs or databases. In `postgres_board`, save an edit, reconnect, reload the board and verify identity, ordering and values. Include a failed-write case proving no partial edit is visible. Never skip the test on a missing database.

### 03-desktop

Use Longbridge GPUI Kit to display the persisted board and edit an item inline. Show pending, saved and failed edits distinctly; retain the draft on failure. Cover accepted and failed saves at the UI boundary in `desktop_edit`. Build the desktop binary and exercise save/close/reopen on synthetic data. Record what was actually observed; block if runtime acceptance is unavailable.

### After the first slice

Build a workflow parity ledger from approved, sanitized behavior descriptions. Track each workflow's reference behavior, Rust owner, automated check and desktop acceptance separately. Queue short packets for columns and views, then imports, conversations, search, automation and integrations according to dependencies. Track API compatibility and mobile delivery explicitly; desktop success does not verify either. Expand the queue only when a packet has an observable outcome and a failure case.
