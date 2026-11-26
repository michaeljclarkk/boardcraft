# Rustday roadmap: full workflow parity

**Goal:** a user can perform every supported workflow with the same inputs, controls, permissions and results, including after a failed save, a disconnect or an upgrade. Persisted records, shared links, API clients and background work are part of that contract. Rust owns the rules, PostgreSQL owns durable state, and Longbridge GPUI Kit supplies the desktop controls.

**Starting point:** the development window and isolated database are available. The first queued product packet is `01-board`. The three packets below are the executable queue; later phases are planned. Use `node scripts/run.mjs status` for verified checkpoints in this checkout.

**Windows desktop and Android are the active targets.** After the first desktop slice, verify the edit on Android: edit an item, background and reopen the app, then confirm the saved value on Windows. Check keyboard and mouse on Windows, and touch, keyboard dismissal, system Back and suspend/resume on Android.

**iOS follows Android.** Keep shared Rust rules and reusable controls portable. Defer iOS implementation and testing until Android's workflow and delivery checks pass and a Mac with Xcode is available. Track iOS as deferred so it does not block Windows or Android milestones. When iOS work begins, run the shared acceptance scenarios on iOS and record simulator or device evidence separately.

## First delivery: edit an item, close the app, see the edit again

Each packet inherits the previous one's behavior. Run Cargo from the repository root. The named integration test targets must contain enabled assertions. Record any uncovered contract in the [workflow register](docs/workflows.md).

### 01-board

Add the board model to the existing Rust workspace. On a synthetic board, create two uniquely identified items with the same title. Rename one by identity: its order and the other item must not change. Editing a missing item returns a recoverable outcome and leaves the board intact. Cover both paths in the `board_edit` integration test. No database or renderer needed for these rules.

### 02-postgres

Add migrations and persist the board operation in PostgreSQL. Test on a dedicated, disposable Rustday database supplied as `RUSTDAY_TEST_DATABASE_URL`. No fallback to other URLs or databases. In `postgres_board`, save an edit, reconnect, reload the board and verify identity, ordering and values. Include a failed-write case proving no partial edit is visible. Never skip the test on a missing database.

Use nonempty UUIDs for durable board and item identities so the store can round-trip existing IDs. Keep the rename scoped to its board. For interactive edits, store the trimmed title; reject blank titles and titles over 255 UTF-16 code units after trimming. Exercise these rules with synthetic inputs and distinguish rejected edits from database faults. Historical imports have a separate spelling contract.

### 03-desktop

Use Longbridge GPUI Kit to display the persisted board and edit an item inline. Show pending, saved and failed edits distinctly; retain the draft on failure. Cover accepted and failed saves at the UI boundary in `desktop_edit`. Build the desktop binary and exercise save/close/reopen on synthetic data. Record what was actually observed; block if runtime acceptance is unavailable.

## Delivery phases after the first slice

Each row is a group of independently testable packets. For each area, inventory the user actions, persisted settings, contracts and supported clients **before** estimating it. Add newly identified behavior to the register even if it falls between phases. The exit scenarios below are representative gates, not the entire inventory.

| Order | Deliver in this order | Exit scenario |
|---|---|---|
| 1. Identity, access and navigation | Sign-in and sessions → users, roles and teams → workspace/board access → sidebar, stable destinations, Back/Forward and copy-link actions. | Two users open the same deep link. Each sees exactly what their role allows. Following a link does not grant access; a denied write leaves the board unchanged. |
| 2. Board structure and grid | Workspaces and folders → boards, groups, items and subitems → selection, ordering, moves, bulk actions and lifecycle operations. Match grid density, scroll position, keyboard focus, menus, drag targets and draft retention. | Move a mixed selection, reload, then open an item by its original link. Verify IDs, order and focus; exercise an unavailable destination and a conflicting edit. |
| 3. Columns and relationships | Inventory every column type. Implement setup, defaults, validation, editors and display; then formulas, linked records, derived values and file cells. Reuse the same rules for direct edits, batch writes and imports. | For every type, create, edit, clear and reload a value. Include empty versus zero, date boundaries, renamed labels and unavailable linked items where applicable. Verify stored and displayed results separately. |
| 4. Views, charts and forms | Saved layouts → filtering, sorting and grouping → chart settings and drill-down → form configuration, validation and submission. Preserve shared versus personal settings. | Two users reopen a configured view and see the intended rows, ordering and settings. A valid form creates the expected item once; an invalid submission preserves input without creating an item. |
| 5. Imports, exports and history | Spreadsheet and archive formats → URL acquisition → mapping and relationship resolution → preview, progress, cancellation, retry and publication. Preserve historical values separately from editable current state. | Import a synthetic board with duplicate labels, attachments and cross-board links. Interrupt and retry it; verify row counts, identities, history and links. Reject a malformed file without exposing partial results. Check export/reimport for each supported round trip. |
| 6. Collaboration and discovery | Item conversations, replies, mentions and files → workspace chat and activity → inbox and notifications → personal work, quick search, global search and preferences. | Mention a teammate, open the notification on another client, and navigate to the intended item. Check read state after reconnect, file preview failure, and search visibility for a user without access. |
| 7. Automations and integrations | Triggers, conditions, action graphs and configuration → durable scheduling and retries → provider adapters, inbound events and delivery status → assistant conversations, tools and recovery. | Commit a change, restart the worker, and redeliver the same synthetic event. Verify the intended effect occurs once where guaranteed. Show a failed or ambiguous delivery distinctly from the saved edit; recover an interrupted assistant operation. |
| 8. Client and API completion | Verify each applicable workflow on desktop, web, mobile and programmatic clients as it lands. Close remaining gaps in GraphQL operations, compatibility APIs, tool contracts, live refresh, mobile navigation, drafts, files and settings. | Run the same synthetic journey through each supported entry point. Compare values, IDs, errors, access decisions and side effects. Include concurrent edits, an independent tab, and a disconnected mobile client returning with a draft. |
| 9. Upgrade and delivery | Database migrations, configuration, backup/restore → desktop installation and update → hosted service and mobile packages → final regression run. | Upgrade a populated synthetic installation. Reopen saved views and links, verify identities, attachments and pending work, then restore a backup into a clean instance and rerun the critical journeys. |

## Turn a phase into runnable work

1. Pick the smallest unfinished user journey whose dependencies are verified. Give it a stable packet ID, synthetic fixture, expected visible **and** stored outcome, failure case and named check. For example, split column work by behavior family rather than attempting the entire column catalogue in one chat.
2. Add its acceptance criteria here and its gate to the runner before dispatch. Use one fresh session per packet. Aim for 10,000–20,000 changed lines for large commits; split earlier at a tested boundary and never exceed 20,000. Small packets stay small.
3. Start with a Kit control. If it cannot support a required interaction, put the reusable control in `rustday-ui` with a public API, synthetic example and interaction test. Keep board operations and persistence in the app.
4. Finish with a handoff linking the rule owner, fixture, checks run, observed results, remaining gaps and one next action. A blocked packet remains open. Re-run affected earlier journeys when a shared rule changes.

## Release gate: when can we call it 1:1?

- Every inventoried workflow has a row with a reproducible fixture, expected outcome, passing check and evidence for each applicable client. Label gaps **unknown**, **planned**, **implemented** or **verified**; only the last counts toward completion. A deliberate behavior difference stays visible as a difference.
- Exercise complete journeys as well as isolated features: import → edit → save a view → share a link; submit a form → run an automation → open the notification; lose connection during an edit → reconnect → resolve and reopen. Check permissions and durable state at each boundary.
- Verify interactive fidelity in running clients: layout, keyboard and pointer actions, touch where supported, focus, scrolling, loading, empty, error and recovery states. A passing Rust test alone cannot close a UI row.
- Reconcile the final workflow, platform, API, data-format and delivery inventories. Any missing row, unresolved discrepancy or untested applicable client keeps the parity milestone open.
