# Roadmap and next task

Updated 2026-11-28. This file holds the one task to work on next. The delivery order lives in [ROADMAP.md](../ROADMAP.md#delivery-order-owner-direction-2026-11-28); the dated history lives in its [progress log](../ROADMAP.md#progress-log).

**Only job:** a 1:1 Rust port of the whole product's C# and React/TSX surface. Write code, run it, test the UI with Computer Use and GPUI Kit tests. No harnesses, orchestrators, packets or new infrastructure.

## Next task

**Finish this workspace-navigation screen's same-fixture parity comparison (Rust GPUI/WASM).** Search and temporary hover/pin/dismiss were rewritten in `crates/web/src/navigation.rs`; six production-view GPUI interaction tests and live Chrome/Computer Use checks now cover filtering, focus, collapse and temporary hover without table shift. The bounded before-change viewer exposed a duplicate-hover panic; the after-change viewer rendered and the panic is fixed. Remaining differences and observations are in `docs/workflows.md`: different fixtures prevent board-order/pixel parity claims; slide/reduced-motion handling, `aria-controls`, menu/dialog hover exclusions, compact drawer/touch and active-edit comparison remain open. Workspace scoping/reset has GPUI coverage but no live multi-workspace comparison. Use the bounded viewer command in `docs/development.md` for the next comparison; the most recent relaunch returned `program not found` after concurrent tools changes, so confirm launch first without restoring any runner. Do not start another screen. Keep board journey criteria below open where not demonstrated.

Acceptance:

- The same fixture shows the same boards in the same order.
- Opening a board shows the same table, groups, controls and labels as the internal tool at the same window size.
- Rename works with double-click and F2; Escape cancels; Enter or focus loss saves.
- The saved value survives closing and reopening the app.
- Failure states (validation, unavailable, conflict, retry) look and behave like the internal tool.
- Any remaining difference is written in [workflows.md](workflows.md), not left in chat.
- No harness, driver, orchestrator, packet runner, new crate, dependency, abstraction or infrastructure is added. The change is the screen only.

Finish this journey, record it, and stop. Do not start group management, auth or Android.

## Recently delivered

- **2026-11-28 — Agent setup reset.** Product-first `AGENTS.md`, workflow + UI parity as equal criteria, 100% Rust restated, coordinator/packet gates removed, queue runner removed, custom UI harnesses retired, delivery order resliced. No application code changed. Details in the [progress log](../ROADMAP.md#progress-log).

## Open gaps

[gaps.md](gaps.md) is the current list. Read it before choosing work.
