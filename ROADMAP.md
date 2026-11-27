# boardcraft roadmap: internal-tool parity

**Goal:** a user can perform every supported internal-tool workflow in boardcraft with the same inputs, controls, layout, permissions and results, including after a failed save, a disconnect or an upgrade. Both workflow parity and UI parity are acceptance criteria. Rust owns the rules, PostgreSQL owns durable state, and Longbridge GPUI Kit supplies the desktop controls.

## Delivery order (owner direction, 2026-11-28)

Ship one vertical desktop journey at a time. Do not start the next until the current one is demoable in the running app with the internal tool open beside it.

1. **Board journey.** Launch the desktop app, see the boards available in the running app, open a board, rename an item, restart and confirm the saved name. The screen, controls, labels and states match the internal tool.
2. **Groups and items.** Groups are visible; create, rename, collapse and reorder them. Create, move, select and reorder items. Match the internal tool's table, menus and drag behaviour.
3. **Board chrome.** Navigation, share link, archive/trash and the recovery states around them.
4. **Auth.** Sign-in, resolve, logout and session restore. This comes after the board works and only matters before shared deployment.
5. **Other clients.** Android and the browser mirror the desktop journey once desktop is right.

Packets 01–57 are the historical port index below; they are reference material, not gates. The queue runner and agent dispatch were removed; earlier handoff files are gone too. Packets 13–21 (auth) and 26–56 (browser and the retired test harness) are out of delivery order and stay closed until steps 4 and 5. Handoffs that mention harnesses, drivers or OCR describe retired tooling; do not rebuild them.

## Current task

**Board journey.** The current owner request targets Rust GPUI/WASM. Open a board from the list, rename an item, then restart the client and show the saved value. Use the [UI port checklist](plan/ui-port-checklist.md) and record differences in [docs/workflows.md](docs/workflows.md). Desktop acceptance is still open. Details: [docs/roadmap.md](docs/roadmap.md#next-task).

## Progress log

Newest first. After each delivery, add one dated entry: what changed, what you saw in the app, what stayed unverified, and the next action. Link the feature note or files. This is the project's memory; do not move it into chat.

**2026-11-28 — Peek at the collapsed workspace navigation.** Implemented the temporary hover flyout on the collapsed WASM workspace rail in `crates/web/src/shell/workspace.rs`: pointer enter peeks after 140ms on hover-capable pointers, leave dismisses after 220ms, a pending timer is cancelled by the opposite transition, `(hover: hover)` required, Escape on a non-search element dismisses, choosing a board dismisses, pin toggle + Close workspace navigation clear peek + restore rail focus, board + URL + workspace search query retained, temporary panel is an absolute overlay that does not shift the table. Behavior ported from the reference temporary-flyout vs pinned-panel model (pointer enter/leave timing, dismiss, pin); only observable contract recorded. WASM `cargo +nightly-2026-10-31 check`, WASM Clippy `-D warnings`, native `boardcraft-web` tests (6) and `cargo fmt --check` passed; `cargo clippy --workspace --all-targets -D warnings` has a **pre-existing** unrelated `useless_conversion` in `crates/boardcraft-ui/examples/heading_drag.rs`. **Not verified live this run:** Chrome DevTools MCP / Computer Use could not be invoked; disposable viewer serves `http://127.0.0.1:54917/web/` but mouse + keyboard observation was not performed, so no visual parity is claimed. Remaining known differences (no 300ms transition/`prefers-reduced-motion`, no `aria-controls`, no document-level `pointerover` rescan + menu/dialog exclusions, no compact/overlay drawer mode) recorded in `docs/workflows.md`. Next: live-verify on disposable viewer with mouse + keyboard. [Details](docs/workflows.md).

**2026-11-28 — Collapse and reopen workspace navigation.** In Chrome the chevron hid the filtered sidebar beside Review; Enter on Workspace reopened the same `rev` query without navigating away. The WASM build and target Clippy passed. The viewer now offers bounded sessions that expire without terminal input. Next: the temporary navigation flyout. [Checks and remaining differences](docs/workflows.md).

**2026-11-28 — Find a board in the WASM sidebar.** The search toggle and local filter are implemented. Chrome showed `No matching boards.` for the same query in both apps; in the viewer Escape restored the full list and focused the toggle. A mixed-case substring returned only Backlog; Tab and Enter opened it. Six web-crate tests and WASM Clippy passed. Fixture, theme and workspace-scoping differences remain. Next: collapse and reopen the workspace navigation.

**2026-11-28 — WASM failed rename.** A disconnected rename now shows its retained change above the groups. In Chrome, a failed retry read sent no write; restoring connectivity and clicking Retry saved only the second Review. The host reads just that ID and rechecks authorization on write. Next: workspace board search in WASM. [Runtime evidence](docs/workflows.md). Reference fixture comparison and remaining board UI differences stay open.

**2026-11-28 — WASM board list → rename → reopen.** The viewer opens a two-board list with persistent navigation, a selected title and compact grouped name cells. In Chrome the second Review kept its new name after closing and reopening the tab; the first Review and Sketch stayed unchanged. F2 and whitespace rejection on blur worked. The debug viewer initially trapped while laying out a board; optimizing the web crate made the same journey render. Three GPUI tests, four board-domain tests and 43 host/DB tests passed. Next: match board-level rename recovery and compare conflict/retry on the same fixture. [Workflow evidence and remaining layout gaps](docs/workflows.md).

**2026-11-28 — Agent setup reset.** Rustday now runs on the internal tool's agent model: direct owner authority, workflow **and** UI parity as equal acceptance, 100% Rust restated, queue runner removed, no custom UI harness, and a delivery order that starts with the desktop board journey. Changed `AGENTS.md`, `opencode.json`, `ROADMAP.md`, `README.md`, `docs/README.md`, `docs/workflows.md`, `docs/roadmap.md`, `docs/gaps.md` and the project skills. No application code changed. Next: build the desktop board journey.

The numbered packets below are the historical milestone index; there is no queue command to run them. A checkpoint from the former browser client is historical; wasm parity needs its own evidence.

**Windows desktop and Android are the active targets.** After the first desktop slice, verify the edit on Android: edit an item, background and reopen the app, then confirm the saved value on Windows. Check keyboard and mouse on Windows, and touch, keyboard dismissal, system Back and suspend/resume on Android.

**iOS follows Android.** Keep shared Rust rules and reusable controls portable. Defer iOS implementation and testing until Android's workflow and delivery checks pass and a Mac with Xcode is available. Track iOS as deferred so it does not block Windows or Android milestones. When iOS work begins, run the shared acceptance scenarios on iOS and record simulator or device evidence separately.


## Decided stack change (2026-11-27)

The browser client is GPUI on WebAssembly. [Launch the Rust viewer](docs/development.md#browser-viewer) and open the printed URL in an ordinary browser. Use Computer Use for live journeys and GPUI Kit tests for control behavior.

## Historical index

The packet files were deleted. Use git history if you need an old brief; the progress log records what shipped.

## Longer-term shape

After the desktop journey, follow the [delivery order](#delivery-order-owner-direction-2026-11-28): groups and items, then board chrome, then auth, then Android and the browser. Later areas - columns, views, imports, collaboration and automations - enter the order as the desktop app reaches them, always with the same rule: one visible journey, the internal tool open beside it, workflow and UI parity together.

Do not maintain a second phase table or convert areas into packets. The next task lives in [docs/roadmap.md](docs/roadmap.md#next-task); the history lives in the [progress log](#progress-log).

## Release gate: when can we call it 1:1?

- Every inventoried workflow has a row with a reproducible fixture, expected outcome, passing check and evidence for each applicable client. Label gaps **unknown**, **planned**, **implemented** or **verified**; only the last counts toward completion. A deliberate behavior difference stays visible as a difference.
- Exercise complete journeys as well as isolated features: import → edit → save a view → share a link; submit a form → run an automation → open the notification; lose connection during an edit → reconnect → resolve and reopen. Check permissions and durable state at each boundary.
- Verify interactive fidelity in running clients: layout, keyboard and pointer actions, touch where supported, focus, scrolling, loading, empty, error and recovery states. A passing Rust test alone cannot close a UI row.
- Browser rows require Computer Use observations on the GPUI wasm client. Evidence captured with a retired driver is historical and does not close the row.
- Reconcile the final workflow, platform, API, data-format and delivery inventories. Any missing row, unresolved discrepancy or untested applicable client keeps the parity milestone open.
