---
name: boardcraft-ui-parity
description: Match a boardcraft screen to the internal tool's UI while porting a workflow. Use when building or reviewing layout, controls, states, keyboard/focus or pointer/touch behaviour.
---

# UI parity

UI parity is an acceptance criterion alongside workflow parity, not a polish pass.

## Compare like for like

- Open the internal tool and boardcraft with the same fixture, same window size and same zoom.
- Capture both screens before and after the change.
- Compare structure first, then controls, then states, then motion.

## Checklist

- **Structure:** panels, header, toolbar, table and item layout in the same place and order.
- **Controls:** same control types for the same action. A menu stays a menu; a drag stays a drag.
- **Labels and order:** same wording and ordering, using our own names only where the internal tool's came from a third party.
- **Density:** row height, spacing, column widths, truncation and wrapping.
- **Colour and emphasis:** background, borders, selection, hover, focus ring and disabled treatment.
- **States:** empty, loading, saving, saved, validation error, conflict, unavailable and retry look and behave the same.
- **Keyboard and focus:** same shortcuts, tab order, Enter/Escape behaviour and focus return.
- **Pointer and touch:** click, double-click, right-click menus, drag handles, long-press and touch targets.
- **Result:** the same visible outcome after the action, including after restart.

## Rules

- Match the internal tool's layout and behaviour by rebuilding it with Rust and GPUI Kit controls; do not embed its HTML, CSS, JS or XAML.
- Prefer a stock GPUI Kit control that matches. Build a custom control only when none does, and keep it in `boardcraft-ui`.
- Record each remaining difference in [docs/workflows.md](../../../docs/workflows.md) with the screen and state.
- Do not close the workflow while a difference is open. Do not claim parity from tests alone.
