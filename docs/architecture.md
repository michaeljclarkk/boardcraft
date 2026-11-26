# Architecture and owner map

| Boundary | Owner | Responsibility |
|---|---|---|
| Board items | [`Board::rename_item` and `interactive_title`](../crates/board/src/lib.rs) | Own non-nil UUID identities, item order and the shared interactive title rule. |
| Durable boards | [`store`](../crates/board/src/store.rs) and [schema](../crates/board/migrations/0001_boards.sql) | Migrate the schema, round-trip ordered boards, and return a board-scoped rename result after commit. |
| Item revisions | [Revision migration](../crates/board/migrations/0002_item_revision.sql) and [`store::save_title`](../crates/board/src/store.rs) | Start at 1. Under the item lock, check the caller's condition and advance the revision only for a changed title. |
| Mobile board read and name save | [Rust client and wire types](../crates/mobile-edit-client/src/lib.rs) → [loopback handler](../crates/mobile-edit-host/src/lib.rs) → `store::load_items` / `store::rename_item_at_revision` | Serve one synthetic board. The client needs only a loopback port and IDs; the host owns PostgreSQL access and maps errors to HTTP status codes. |
| Desktop board editor | [Kit window and rows](../crates/desktop/src/main.rs), [`InlineEdit`](../crates/desktop/src/lib.rs) | Each item ID has a name button. The row snapshots the accepted spelling on entry and mounts a focused Kit Input. `InlineEdit` retains the draft, accepted title and save phase; changed drafts go through `store::rename_item_if_current` off the UI thread. |
| Android fixture board editor | [`BoardView`](../android/src/lib.rs), [launch arguments](../android/src/connection.rs), [APK build](../android/build-debug.ps1) | One Kit Root owns the board view. Without arguments it opens the offline fixture; with a UUID and loopback port it loads and submits via `EditClient` off the frame thread. The locked mobile host supplies the invisible IME proxy; the Kit input owns the draft. |
| Framework versions | [Desktop package](../crates/desktop/Cargo.toml) and [lockfile](../Cargo.lock) | Select the Kit release as a unit; use its GPUI re-exports rather than adding an independent GPUI revision. |
| Development database | [Local helper](../scripts/postgres.ps1) | Own the disposable local cluster and test login. |
| Milestone execution | [Runner](../scripts/run.mjs) | Dispatch one worker per packet, check its handoff, run gates and record a checkpoint. |

For board edits, the intended path is Kit editor → named Rust operation → PostgreSQL transaction → accepted value back to the editor. The draft remains view state. Add owner links as each boundary lands; avoid creating empty crates to match this diagram.

## Custom GPUI controls

When Kit cannot reproduce a required interaction, put the reusable control in `crates/boardcraft-ui` as a library and list it here. Its public API accepts display values, state and events; it must not depend on the desktop binary, domain entities, database, routing or services. The desktop caller maps control events to boardcraft operations. Include a minimal runnable example using synthetic values, document keyboard/focus behavior and test the interaction that justified the control. A consumer should be able to use the control as a path dependency in another GPUI app. Create the crate with the first real control.
