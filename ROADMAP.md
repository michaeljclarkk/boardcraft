# boardcraft roadmap: full workflow parity

**Goal:** a user can perform every supported workflow with the same inputs, controls, permissions and results, including after a failed save, a disconnect or an upgrade. Persisted records, shared links, API clients and background work are part of that contract. Rust owns the rules, PostgreSQL owns durable state, and Longbridge GPUI Kit supplies the desktop controls.

The numbered packets below are the milestone index. Use `cargo run --locked --manifest-path tools/Cargo.toml -- status` for registered packets and local checkpoints. A checkpoint from the former browser client is historical; wasm parity needs its own evidence.

**Windows desktop and Android are the active targets.** After the first desktop slice, verify the edit on Android: edit an item, background and reopen the app, then confirm the saved value on Windows. Check keyboard and mouse on Windows, and touch, keyboard dismissal, system Back and suspend/resume on Android.

**iOS follows Android.** Keep shared Rust rules and reusable controls portable. Defer iOS implementation and testing until Android's workflow and delivery checks pass and a Mac with Xcode is available. Track iOS as deferred so it does not block Windows or Android milestones. When iOS work begins, run the shared acceptance scenarios on iOS and record simulator or device evidence separately.

## Decided stack change (2026-11-27)

The browser client is GPUI on WebAssembly. [Launch the Rust viewer](docs/development.md#browser-viewer) and open the printed URL in an ordinary browser. Use Computer Use for live journeys and GPUI Kit tests for control behavior. The [runner](tools/src/packets.rs) registers packets through 56, retains the host/store checks and leaves unobserved live outcomes pending. Packet 57's deletion acceptance remains pending.

## Packet index

Each packet inherits the applicable earlier workflows. The browser preview and Android frame/edit/reopen entries below describe historical milestones; their old driver commands are retired. Run Cargo from the repository root. Record any uncovered contract in the [workflow register](docs/workflows.md).

Each packet is one file under [packets](packets/) and is loaded only when dispatched. Read your own packet file, not the whole queue; a full-queue read costs more tokens than the code the packet produces.

- [01-board](packets/01-board.md) — Add the board model to the existing Rust workspace.
- [02-postgres](packets/02-postgres.md) — Add migrations and persist the board operation in PostgreSQL.
- [03-desktop](packets/03-desktop.md) — Use Longbridge GPUI Kit to display the persisted board and edit an item inline.
- [04-mobile-edit-service](packets/04-mobile-edit-service.md) — An Android client needs to read the board again after resume and submit a name without a database connection.
- [05-android-emulator-edit](packets/05-android-emulator-edit.md) — Run the existing Rust EditClient as an Android executable on the dedicated x86_64 Rustday_Edit_X64 AVD.
- [06-android-kit-frame](packets/06-android-kit-frame.md) — Open a synthetic board in an x86_64 **debug APK** and tap the second item's name to enter edit mode.
- [07-android-kit-edit](packets/07-android-kit-edit.md) — Load a disposable two-row board in the Android Kit window using the existing EditClient and production Host.
- [08-cross-client-reopen](packets/08-cross-client-reopen.md) — Run the 07 rejection/correction journey, then distinguish an accepted name from a subsequent unsaved draft across two clients.
- [09-windows-name-cell](packets/09-windows-name-cell.md) — Replace the always-visible Windows inputs with focusable Kit name display controls bound to row IDs.
- [10-windows-name-recovery](packets/10-windows-name-recovery.md) — Translate the board's recovery for a submitted name.
- [11-windows-failure-reopen](packets/11-windows-failure-reopen.md) — Restore a **submitted, failed** Windows name change after remount or fresh process launch.
- [12-windows-name-draft](packets/12-windows-name-draft.md) — Reopen an **unsubmitted** Windows name edit.
- [13-account-session](packets/13-account-session.md) — Begin phase 1 at the account operation boundary.
- [14-loopback-board-access](packets/14-loopback-board-access.md) — Protect the existing single-board loopback host.
- [15-android-kit-sign-in](packets/15-android-kit-sign-in.md) — On connected launch, use real Kit inputs for identifier → Next → masked password → Sign in.
- [16-android-kit-logout](packets/16-android-kit-logout.md) — On a connected Android board, offer a Kit Log out action.
- [17-android-session-restore](packets/17-android-session-restore.md) — Cold launch must resolve the stored session for the exact selected loopback origin before showing protected content.
- [18-windows-connected-login](packets/18-windows-connected-login.md) — Add a connected Windows Kit entry for opening a protected board by loopback port and board ID.
- [19-windows-env-session](packets/19-windows-env-session.md) — Accept an optional existing session from BOARDCRAFT_SESSION_TOKEN only with an exact origin match.
- [20-windows-connected-name](packets/20-windows-connected-name.md) — Edit one item name in boardcraft-desktop connect.
- [21-windows-connected-logout](packets/21-windows-connected-logout.md) — Offer Log out on a connected Windows board, including a read-only board.
- [22-visible-board-list](packets/22-visible-board-list.md) — List the active boards visible to the signed-in Member or Viewer.
- [23-windows-board-navigation](packets/23-windows-board-navigation.md) — Open a board from a connected Windows Kit list without supplying its UUID at launch.
- [24-android-board-navigation](packets/24-android-board-navigation.md) — Start Android Kit with action dev.boardcraft.BROWSE and a loopback port, without a board UUID.
- [25-windows-board-link](packets/25-windows-board-link.md) — From an already opened, read-only board in Windows **browse**, choose **Share**.
- [26-browser-board-preview](packets/26-browser-board-preview.md) — On one local origin a user can sign in, see the boards available to their account and log out.
- [27-browser-board-navigation](packets/27-browser-board-navigation.md) — Open either Review in the signed-in browser preview.
- [28-browser-board-history](packets/28-browser-board-history.md) — Make the **Board** address usable in the loopback preview.
- [29-browser-selected-name](packets/29-browser-selected-name.md) — Rename one item inline on the board opened from the browser list.
- [30-browser-session-resume](packets/30-browser-session-resume.md) — Open a plain board URL in a second independent tab of the same browser session, or reload that tab.
- [31-browser-board-groups](packets/31-browser-board-groups.md) — Open the last Review from the signed-in browser list.
- [32-browser-group-item-create](packets/32-browser-group-item-create.md) — Create one ordinary top-level item at the bottom of a physical group on the selected board.
- [33-browser-group-item-move](packets/33-browser-group-item-move.md) — On selected board 201, move **one existing root item** from the first Work to the last Work.
- [34-browser-group-item-reorder](packets/34-browser-group-item-reorder.md) — In the selected board's unsorted physical grid, drag **one root by its grip** within its group.
- [35-browser-item-selection](packets/35-browser-item-selection.md) — Select roots in the selected board's expanded, unsorted physical groups.
- [36-browser-selection-group-move](packets/36-browser-selection-group-move.md) — Move the previous selection to a physical group on the same board, using the toolbar rather than a row menu.
- [37-browser-root-archive-restore](packets/37-browser-root-archive-restore.md) — On a selected physical board, archive the **second Ink in the first Work** from its row menu, reopen the board's Ar…
- [38-browser-group-archive-restore](packets/38-browser-group-archive-restore.md) — Archive one **physical group**, keeping its roots in place.
- [39-browser-group-create](packets/39-browser-group-create.md) — On a selected physical board, choose **New item options → New group of items**.
- [40-browser-group-edit](packets/40-browser-group-edit.md) — Edit the name and colour of **one existing active physical group** on the selected browser board.
- [41-browser-group-reorder](packets/41-browser-group-reorder.md) — Move one **active physical group one place** on the selected browser board.
- [42-browser-group-heading-drag](packets/42-browser-group-heading-drag.md) — On the selected browser board, an Owner can drag a **physical group's name** past several sections in one move.
- [43-browser-group-collapse](packets/43-browser-group-collapse.md) — Fold and reopen physical groups on the selected board's **main table**.
- [44-windows-main-table](packets/44-windows-main-table.md) — Open the grouped main table in Windows browse and share its saved folds.
- [45-windows-grouped-item-entry](packets/45-windows-grouped-item-entry.md) — Add a root in a selected physical group and edit its name inline in Windows browse.
- [46-windows-selection-move](packets/46-windows-selection-move.md) — Select roots and move them together to a physical group in Windows browse.
- [47-windows-board-archive](packets/47-windows-board-archive.md) — Archive and restore roots and physical groups in Windows browse.
- [48-windows-group-fields](packets/48-windows-group-fields.md) — Create a group and edit its name and colour in Windows browse.
- [49-windows-group-order](packets/49-windows-group-order.md) — Reorder physical groups by menu or drag in Windows browse.
- [50-windows-item-placement](packets/50-windows-item-placement.md) — Move an item to another group or drag it into position in Windows browse.
- [51-selected-root-archive](packets/51-selected-root-archive.md) — Archive selected items together from the Windows and browser toolbars.
- [52-board-item-trash](packets/52-board-item-trash.md) — Move selected items to Trash and restore them in Windows and browser.
- [53-web-shell](packets/53-web-shell.md) — Build the GPUI wasm shell in `crates/web` and serve it from the loopback host.
- [54-web-auth-board](packets/54-web-auth-board.md) — Sign in, list, open, rename and recover a rejected save in the wasm client.
- [55-web-groups](packets/55-web-groups.md) — Groups, create, move, reorder, selection, archive and Trash in the wasm client.
- [56-web-test-harness](packets/56-web-test-harness.md) — Observe the live canvas with Computer Use and compare the results with independent host/store checks.
- [57-delete-js-preview](packets/57-delete-js-preview.md) — Completely remove the JavaScript preview, its routes, feature, example and harness after parity is verified.

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
| 8. Client and API completion | Verify each applicable workflow on desktop, web, mobile and programmatic clients as it lands. Close remaining gaps in GraphQL operations, compatibility APIs, tool contracts, live refresh, mobile navigation, drafts, files and settings. | Run the same synthetic journey through each supported entry point. Compare values, IDs, errors, access decisions and side effects. Include concurrent edits, an independent tab, and a disconnected mobile client returning with a draft. Browser work targets the GPUI wasm client in `crates/web`; evidence from the JavaScript preview is transitional ([decision](docs/web-client-stack.md)). |
| 9. Upgrade and delivery | Database migrations, configuration, backup/restore → desktop installation and update → hosted service and mobile packages → final regression run. | Upgrade a populated synthetic installation. Reopen saved views and links, verify identities, attachments and pending work, then restore a backup into a clean instance and rerun the critical journeys. |

## Turn a phase into runnable work

1. Pick the smallest unfinished user journey whose dependencies are verified. Give it a stable packet ID, synthetic fixture, expected visible **and** stored outcome, failure case and named check. For example, split column work by behavior family rather than attempting the entire column catalogue in one chat.
2. Write its acceptance criteria in `packets/<packet-id>.md` and add its gate to the runner before dispatch. Use one fresh session per packet. Aim for 10,000–20,000 changed lines for large commits; split earlier at a tested boundary and never exceed 20,000. Merge specs smaller than one workflow instead of dispatching them separately.
3. Start with a Kit control. If it cannot support a required interaction, put the reusable control in its own crate with a public API, synthetic example and interaction test. Keep board operations and persistence in the app.
4. Finish with a handoff linking the rule owner, fixture, checks run, observed results, remaining gaps and one next action. A blocked packet remains open. Re-run affected earlier journeys when a shared rule changes.

## Release gate: when can we call it 1:1?

- Every inventoried workflow has a row with a reproducible fixture, expected outcome, passing check and evidence for each applicable client. Label gaps **unknown**, **planned**, **implemented** or **verified**; only the last counts toward completion. A deliberate behavior difference stays visible as a difference.
- Exercise complete journeys as well as isolated features: import → edit → save a view → share a link; submit a form → run an automation → open the notification; lose connection during an edit → reconnect → resolve and reopen. Check permissions and durable state at each boundary.
- Verify interactive fidelity in running clients: layout, keyboard and pointer actions, touch where supported, focus, scrolling, loading, empty, error and recovery states. A passing Rust test alone cannot close a UI row.
- Browser rows require Computer Use observations on the GPUI wasm client. Evidence captured with a retired driver is historical and does not close the row.
- Reconcile the final workflow, platform, API, data-format and delivery inventories. Any missing row, unresolved discrepancy or untested applicable client keeps the parity milestone open.
