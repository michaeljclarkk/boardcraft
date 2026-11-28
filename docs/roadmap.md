# Roadmap and next task

Updated 2026-11-28. This file holds the one task to work on next. The delivery order lives in [ROADMAP.md](../ROADMAP.md#delivery-order-owner-direction-2026-11-28); the dated history lives in its [progress log](../ROADMAP.md#progress-log).

**Only job:** a 1:1 Rust port of the whole product's C# and React/TSX surface. Write code, run it, test the UI with Computer Use and GPUI Kit tests. No harnesses, orchestrators, packets or new infrastructure.

## Next task

**Finish shared board-screen parity across Rust Windows, WASM and Android.** The owner explicitly requested continued parallel UI work and then the same Rust UI on mobile. The shared title, tabs, toolbar and row components now live in `crates/boardcraft-ui/src/board_chrome.rs` and are consumed by all three clients; each still owns transport and unfinished screen behavior. Browser compact navigation, motion/reduced motion, click occlusion and toolbar name search are implemented. Current observed journeys and limitations are in `docs/workflows.md`. Continue the same board screen: align client search/selection behavior, remaining controls, mobile grouped data and theme/density against the same reference fixture. Keep the GPUI/WASM shell and existing native/mobile clients; do not introduce another view layer or runner.

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
