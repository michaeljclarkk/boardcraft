# Roadmap and next task

Updated 2026-11-28. This file holds the one task to work on next. The delivery order lives in [ROADMAP.md](../ROADMAP.md#delivery-order-owner-direction-2026-11-28); the dated history lives in its [progress log](../ROADMAP.md#progress-log).

**Only job:** a 1:1 Rust port of the whole product's C# and React/TSX surface. Write code, run it, test the UI with Computer Use and GPUI Kit tests. No harnesses, orchestrators, packets or new infrastructure.

## Next task

**Live-verify the peeked collapsed workspace navigation (Rust GPUI/WASM).** The temporary hover flyout is implemented in `crates/web/src/shell/workspace.rs` (`hover_navigation`, `dismiss_peek`, `set_navigation_open`, absolute overlay `sidebar`): pointer enter peeks after 140ms on hover-capable pointers, leave dismisses after 220ms, pending timer cancelled by the opposite transition, Escape on a non-search element dismisses, choosing a board dismisses, pin toggle + **Close workspace navigation** clear peek, `(hover: hover)` required, board + URL + workspace search query retained, temporary panel does not shift the table. Remaining step: run `cargo run --locked --manifest-path tools/Cargo.toml -- viewer --for-seconds 180`, open Chrome, and verify on the same fixture with **mouse + keyboard** against the reference (presentation controls only on the reference): same boards same order; no table shift; board/URL/search retained; temporary peek vs pinned; dismiss paths; then record actual observations + remaining differences in `docs/workflows.md`. Current known differences recorded in `docs/workflows.md` (no 300ms transition / `prefers-reduced-motion`; no `aria-controls`; no document-level `pointerover` rescan + menu/dialog exclusions; no compact/overlay mode). Keep board journey criteria below open where not yet demonstrated.

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
