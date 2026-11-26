# Workflow and gap register

The full product is in scope. The rows below start the first packet; they are not an exhaustive inventory. Before queuing each remaining area, map its screens, native and API consumers, persisted formats, permissions, background effects and failures to synthetic acceptance cases. Cover workspaces, boards and groups, column types, views and forms, relationships, imports/exports, search, conversations/files, workspace chats, activity, personal work, teams/accounts, settings, assistant workflows, notifications, integrations, automations and mobile flows. Reconcile this inventory against the approved reference; an unlisted workflow remains in scope. Record private evidence pointers separately from public acceptance criteria.

Use one row per observable user action. A planned test name is not evidence. When an implementation lands, link the rule owner, the automated check and the runtime observation in the packet handoff.

| Workflow | Acceptance target | Implementation / automated evidence | Desktop evidence or gap |
|---|---|---|---|
| Rename an item | Two equal titles remain distinct. Rename by ID without reordering; a missing ID leaves the board unchanged. | Implemented: [`Board::rename_item`](../crates/board/src/lib.rs); [synthetic fixture and checks](../crates/board/tests/board_edit.rs); [packet evidence](../handoffs/01-board.json). | Windows and Android editor checks remain open. |
| Keep the edit after restart | A fresh connection reads the trimmed title under the same UUID and in the same order. Rejected edits and commit failures preserve the last saved state. | Implemented: [`store::rename_item`](../crates/board/src/store.rs); [PostgreSQL reconnect and rollback fixture](../crates/board/tests/postgres_board.rs); [packet evidence](../handoffs/02-postgres.json). | App close/reopen on Windows and suspend/resume on Android remain open. |
| Recover a failed inline edit | Preserve the draft on failure; show saved state only after commit. | Planned: `03-desktop`, `desktop_edit`. | Exercise save, failure, retry and reopen in the running app. |

The persistence fixture covers board scope and interactive titles (trim, reject blank, at most 255 UTF-16 code units). Snapshot creation and loading preserve existing spelling; historical import behavior remains open. Revision validation, stale field revisions, active groups, write permissions and field editability also remain open.
