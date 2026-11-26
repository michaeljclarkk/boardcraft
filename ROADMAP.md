# boardcraft roadmap: full workflow parity

**Goal:** a user can perform every supported workflow with the same inputs, controls, permissions and results, including after a failed save, a disconnect or an upgrade. Persisted records, shared links, API clients and background work are part of that contract. Rust owns the rules, PostgreSQL owns durable state, and Longbridge GPUI Kit supplies the desktop controls.

The numbered packets below are the executable queue; later phases are planned. Use `node scripts/run.mjs status` for verified checkpoints in this checkout.

**Windows desktop and Android are the active targets.** After the first desktop slice, verify the edit on Android: edit an item, background and reopen the app, then confirm the saved value on Windows. Check keyboard and mouse on Windows, and touch, keyboard dismissal, system Back and suspend/resume on Android.

**iOS follows Android.** Keep shared Rust rules and reusable controls portable. Defer iOS implementation and testing until Android's workflow and delivery checks pass and a Mac with Xcode is available. Track iOS as deferred so it does not block Windows or Android milestones. When iOS work begins, run the shared acceptance scenarios on iOS and record simulator or device evidence separately.

## First delivery: edit an item, close the app, see the edit again

Each packet inherits the previous one's behavior. Run Cargo from the repository root. The named integration test targets must contain enabled assertions. Record any uncovered contract in the [workflow register](docs/workflows.md).

### 01-board

Add the board model to the existing Rust workspace. On a synthetic board, create two uniquely identified items with the same title. Rename one by identity: its order and the other item must not change. Editing a missing item returns a recoverable outcome and leaves the board intact. Cover both paths in the `board_edit` integration test. No database or renderer needed for these rules.

### 02-postgres

Add migrations and persist the board operation in PostgreSQL. Test on a dedicated, disposable boardcraft database supplied as `RUSTDAY_TEST_DATABASE_URL`. No fallback to other URLs or databases. In `postgres_board`, save an edit, reconnect, reload the board and verify identity, ordering and values. Include a failed-write case proving no partial edit is visible. Never skip the test on a missing database.

Use nonempty UUIDs for durable board and item identities so the store can round-trip existing IDs. Keep the rename scoped to its board. For interactive edits, store the trimmed title; reject blank titles and titles over 255 UTF-16 code units after trimming. Exercise these rules with synthetic inputs and distinguish rejected edits from database faults. Historical imports have a separate spelling contract.

### 03-desktop

Use Longbridge GPUI Kit to display the persisted board and edit an item inline. Show pending, saved and failed edits distinctly; retain the draft on failure. Cover accepted and failed saves at the UI boundary in `desktop_edit`. Build the desktop binary and exercise save/close/reopen on synthetic data. Record what was actually observed; block if runtime acceptance is unavailable.

### 04-mobile-edit-service

An Android client needs to read the board again after resume and submit a name without a database connection. Deliver just those two calls over loopback HTTP with a Rust client and host. Run the host for one synthetic board on an ephemeral port against `rustday_test`; keep SQL, credentials and unexpected fault details on the host. No deployment or general-purpose API. Reuse the existing board/store title rule and transaction. A read returns ordered IDs, accepted titles and positive item revisions. A name-only write identifies the board and item, supplies the revision from the read, and returns the committed ID, title and revision. A changed title advances the revision, a repeated value does not. Preserve desktop's existing conditional-save behavior; any changed title written there must also advance the stored revision.

Gate: `cargo test --locked --workspace --test mobile_edit_service`. Use actual serialized requests, the production request handler and a disposable PostgreSQL fixture, with two enabled top-level cases:

- `rename_then_reconnect`: load two `Review` rows, rename only the second to ` Review artwork ` and check the trimmed response and advanced revision. Drop the client, reconnect with a fresh one and verify the ordered IDs, titles and revisions. Independently reload the same board via the store used by Windows. Replay the name-only request from its original positive revision: an older known revision is valid for this scalar edit, and repeating the accepted value must leave its revision unchanged.
- `reject_then_retry`: submit a blank draft and then an impossible future revision. Distinguish validation from conflict; neither may change either stored row or revision. A request scoped to the wrong board cannot rename the item. Re-read the open item, correct and retry successfully. Reject missing items rather than creating them. Never skip on a missing DB or replace the host with a mock.

This is host/client evidence only. The [Android host assessment](docs/development.md#android-native-host) explains the dependency gap. Keep the next runtime gate open: on a dedicated emulator, use a real Kit name input, exercise failed save/correction, keyboard dismissal and system Back, save, background and resume, then open the same UUID in a Windows Kit window. Verify both rows there. Neither SQL readback nor an APK build proves that journey.

### 05-android-emulator-edit

Run the existing Rust `EditClient` as an Android executable on the dedicated x86_64 `Rustday_Edit_X64` AVD. No renderer is needed for this packet. The Windows test starts the 04 production `Host` on loopback for a fresh board/schema in `rustday_test`. Push the cross-built client probe to a unique `/data/local/tmp` path; use `adb -s <serial> reverse --no-rebind tcp:0 tcp:<host-port>` and pass the allocated **device** port to the probe. Only synthetic fixture values cross the bridge; never pass database configuration to Android. Do not use an HTTP stub, host-native client or canned probe output to satisfy an assertion.

Gate: build `android-edit-probe` in `boardcraft-mobile-edit-client` for `x86_64-linux-android`, then run the host's explicitly enabled `android_emulator_edit` suite (commands in the [runbook](docs/development.md#android-emulator-edit-gate)). Each test uses two distinct `Review` items. Assert actual probe results against the fixture, including UUIDs, ordered titles and revisions, and independently reconnect via the store used by Windows to the **same board UUID**:

- `emulator_rename_then_reopen`: Android reads both rows, uses the second ID and its positive revision to submit ` Review artwork `, and receives that ID, trimmed title and advanced revision. Exit the device process. Launch a fresh Android process and read the same board; compare both rows to the committed result, including the unchanged first row. Repeat that comparison via the host store.
- `emulator_reject_then_correct`: Android submits a whitespace-only edit to the second item and receives `InvalidDraft`. Before correction, confirm both original rows and revisions on Android and via a fresh store connection. Use the reopened row's revision to submit ` Corrected `; assert committed `Corrected` under the original second ID, advanced revision and unchanged sibling/order in a fresh device read and host store read.

Direct test invocation must also check the explicit emulator serial, exact AVD name, completed boot and x86_64 ABI **before pushing, reversing ports or running the probe**. Missing prerequisites, timeout, nonzero exit, empty/malformed output or mismatched state fail the gate. Bound every subprocess (≤30 seconds per adb call) and fixture I/O; clean up only the test's own reverse, executable, host and fixture even on errors. Do not touch another AVD, phone, global adb server or shared mappings. Keep portable workspace tests enabled; use a `required-features` test target for the separate emulator suite, never a passing early return.

This proves Android process execution and persisted cross-client data, not an Android app. Next native milestone: render a real Kit input in a compatible Android activity, then exercise correction, keyboard/Back, background/resume and same-board Windows **window** readback. Leave those workflows open.

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
3. Start with a Kit control. If it cannot support a required interaction, put the reusable control in `boardcraft-ui` with a public API, synthetic example and interaction test. Keep board operations and persistence in the app.
4. Finish with a handoff linking the rule owner, fixture, checks run, observed results, remaining gaps and one next action. A blocked packet remains open. Re-run affected earlier journeys when a shared rule changes.

## Release gate: when can we call it 1:1?

- Every inventoried workflow has a row with a reproducible fixture, expected outcome, passing check and evidence for each applicable client. Label gaps **unknown**, **planned**, **implemented** or **verified**; only the last counts toward completion. A deliberate behavior difference stays visible as a difference.
- Exercise complete journeys as well as isolated features: import → edit → save a view → share a link; submit a form → run an automation → open the notification; lose connection during an edit → reconnect → resolve and reopen. Check permissions and durable state at each boundary.
- Verify interactive fidelity in running clients: layout, keyboard and pointer actions, touch where supported, focus, scrolling, loading, empty, error and recovery states. A passing Rust test alone cannot close a UI row.
- Reconcile the final workflow, platform, API, data-format and delivery inventories. Any missing row, unresolved discrepancy or untested applicable client keeps the parity milestone open.
