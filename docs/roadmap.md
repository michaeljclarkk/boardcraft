# Roadmap and next task

Updated 2026-11-28. This file holds the one task to work on next. The delivery order lives in [ROADMAP.md](../ROADMAP.md#delivery-order-owner-direction-2026-11-28); the dated history lives in its [progress log](../ROADMAP.md#progress-log).

**Only job:** a 1:1 Rust port of the whole product's C# and React/TSX surface. Write code, run it, test the UI with Computer Use and GPUI Kit tests. No harnesses, orchestrators, packets or new infrastructure.

## Next task

**Continue Main table native controls and grouped mobile presentation in the UI area.** Windows/browser search timing, matching counts and hidden-selection retention are aligned; Android now renders a Rust port of the MAUI quick-find dock, with offline filtering/Clear/Cancel observed. Next, verify connected Android groups, collapse, selection and Home on the matched board, then translate the remaining native dock filters/item mode and visible board controls from Tuesday. Keep existing commands and drafts. Compare the native Tuesday mobile app in a separate emulator when available; this is a verification step, not an infrastructure project. Current observations and remaining footer, theme, configured-column and navigation differences are in `docs/workflows.md`. Keep the GPUI/WASM shell and existing clients; no new view layer or runner. UI is 38%; scoped native Main table estimates are Windows 40%, Android 30%, not whole-app percentages.

Acceptance:

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
