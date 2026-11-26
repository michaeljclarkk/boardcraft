# Architecture and owner map

| Boundary | Owner | Responsibility |
|---|---|---|
| Board items | [`Board::rename_item` and `interactive_title`](../crates/board/src/lib.rs) | Own non-nil UUID identities, item order and the shared interactive title rule. |
| Durable boards | [`store`](../crates/board/src/store.rs) and [schema](../crates/board/migrations/0001_boards.sql) | Migrate the schema, round-trip ordered boards, and return a board-scoped rename result after commit. |
| Desktop startup | [Desktop entry point](../crates/desktop/src/main.rs) | Initialize GPUI Kit and open the development window. |
| Framework versions | [Desktop package](../crates/desktop/Cargo.toml) and [lockfile](../Cargo.lock) | Select the Kit release as a unit; use its GPUI re-exports rather than adding an independent GPUI revision. |
| Development database | [Local helper](../scripts/postgres.ps1) | Own the disposable local cluster and test login. |
| Milestone execution | [Runner](../scripts/run.mjs) | Dispatch one worker per packet, check its handoff, run gates and record a checkpoint. |

For board edits, the intended path is Kit editor → named Rust operation → PostgreSQL transaction → accepted value back to the editor. The draft remains view state. Add owner links as each boundary lands; avoid creating empty crates to match this diagram.

## Custom GPUI controls

When Kit cannot reproduce a required interaction, put the reusable control in `crates/rustday-ui` as a library and list it here. Its public API accepts display values, state and events; it must not depend on the desktop binary, domain entities, database, routing or services. The desktop caller maps control events to Rustday operations. Include a minimal runnable example using synthetic values, document keyboard/focus behavior and test the interaction that justified the control. A consumer should be able to use the control as a path dependency in another GPUI app. Create the crate with the first real control.
