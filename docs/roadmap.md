# Roadmap and next task

Updated 2026-11-28. This file holds the one task to work on next. The delivery order lives in [ROADMAP.md](../ROADMAP.md#delivery-order-owner-direction-2026-11-28); the dated history lives in its [progress log](../ROADMAP.md#progress-log).

**Only job:** a 1:1 Rust port of the whole product's C# and React/TSX surface. Write code, run it, test the UI with Computer Use and GPUI Kit tests. No harnesses, orchestrators, packets or new infrastructure.

## Next task

**Columns — label management for the existing Status column.** Bulk Change label is implemented and its Windows/browser set, clear, partial failure/retry and reload journeys were observed. Continue from the source Edit labels control in a subsequent focused slice: compare configured labels and the distinct Empty choice before extending the Rust contract. Bulk Android connected touch/focus/recovery and exact visual differences remain in the workflow register. Michael's separate automation-engine agents are working concurrently; do not edit or include their changes.

Android's updated companion reaches sign-in. Connected Status and frozen-name verification remain pending user test sign-in, with no percentage credit. Current estimates: UI 43%, Columns 13%; scoped native Main table Windows 45%, Android 38%. Exact reference sizing, native mobile comparison, configured columns, label management and summaries remain open in the workflow register.
Acceptance:

- **Workflow, UI and UX parity are all required.** Match the same journey, defaults, action order, feedback, keyboard/touch interactions, focus, dismissal, loading, failure/retry and draft preservation. A working operation or a similar screenshot alone does not close a screen. Verify these in the running clients against the reference and record any remaining difference.
- The same fixture shows the same boards in the same order.
- Opening a board shows the same table, groups, controls and labels as the internal tool at the same window size.
- Rename works with double-click and F2; Escape cancels; Enter or focus loss saves.
- The saved value survives closing and reopening the app.
- Failure states (validation, unavailable, conflict, retry) look and behave like the internal tool.
- Any remaining difference is written in [workflows.md](workflows.md), not left in chat.
- No harness, driver, orchestrator, packet runner, new crate, dependency, abstraction or infrastructure is added. The change is the screen only.

Finish this shared board journey and record the observed result. The owner has authorized the same UI on Android in parallel; unrelated group management, auth and infrastructure remain outside this slice.

## Recently delivered

- **2026-11-28 — Agent setup reset.** Product-first `AGENTS.md`, workflow + UI parity as equal criteria, 100% Rust restated, coordinator/packet gates removed, queue runner removed, custom UI harnesses retired, delivery order resliced. No application code changed. Details in the [progress log](../ROADMAP.md#progress-log).

## Open gaps

[gaps.md](gaps.md) is the current list. Read it before choosing work.
