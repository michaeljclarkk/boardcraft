# Shared GPUI components

**Purpose.** `crates/boardcraft-ui` is the shared control space for interactions GPUI Kit does not provide. Desktop, Android and the wasm browser client consume the same controls from it.

## Hard rule: no web fallback

When a required interaction is missing, extend GPUI. A patch MUST NOT work around it with:

- HTML, CSS or hand-written JavaScript in the wasm client;
- React, TSX or any other second view layer;
- a webview, iframe or DOM overlay inside the canvas app;
- a web-only control kept in `crates/web`;
- a "temporary" page served by the host;
- a native-only shortcut that leaves another client without the interaction.

A control that exists on only one client is a missing shared control, not an acceptable platform difference. If GPUI needs a new primitive, build it.

## Where a control goes

- `crates/boardcraft-ui` as a library, listed in the [register](#register).
- Its public API accepts display values, state and events.
- It MUST NOT depend on `crates/desktop`, `crates/board`, the database, routing, services, fixtures or product names.
- The calling client maps control events to boardcraft operations. Domain mapping stays with the client.
- If behaviour cannot be fully generic, split the generic interaction into the control and keep the domain decision outside it.

## Required with every control

| Artifact | Requirement |
|---|---|
| Source and docs | Module doc states the blocked user action, the missing Kit/GPUI capability, the API and the known limits. |
| Example | A runnable synthetic example under `crates/boardcraft-ui/examples/`. |
| Interaction test | Pointer, keyboard and focus behaviour that justified the control. |
| Accessibility | Role, accessible name, state, supported action and keyboard path, documented and tested. |
| Native and wasm | Build and exercise on desktop and `wasm32-unknown-unknown`; record which targets actually ran. |
| Register row | Add the control below with owner, example, test and status. |
| Upstream note | State what Kit/GPUI lacks and what a generic proposal would need. Keep it free of project domain names. |

## Register

| Control | Missing capability | Owner | Example | Interaction test | Wasm | Upstream note |
|---|---|---|---|---|---|---|
| `HeadingDrag` / `SectionBounds` | Kit's generic drag lacks a name-only threshold and heading-midpoint insertion edges | `crates/boardcraft-ui/src/heading_drag.rs` | [`examples/heading_drag.rs`](../crates/boardcraft-ui/examples/heading_drag.rs) | `mod tests` in the owner; desktop grouped-order journeys | Unknown — verify with packet 53 | A generic threshold-and-edge reorder gesture could be proposed to Kit without the group domain. |

## Documenting a missing capability

Keep the note short and reusable:

1. **Blocked action.** What the user must be able to do.
2. **Current limitation.** What Kit/GPUI provides today and why it is insufficient.
3. **Smallest generic API.** The display values, state and events the control needs.
4. **Behaviour and limits.** Thresholds, keyboard path, focus, cancellation and accessibility.
5. **Evidence.** Native and wasm checks that actually ran.
6. **Upstream shape.** What would change in a generic proposal. No project names, records or screenshots.

## Review gate

Reject a change that:

- adds HTML/CSS/JS/React/TSX, a webview, an iframe or a DOM overlay to work around GPUI;
- keeps a web-only control in `crates/web` or a native-only control that leaves another client behind;
- adds a shared control without a synthetic example, an interaction test or an accessibility contract;
- depends on domain entities, storage, routing or services;
- claims wasm support without a recorded build or run.

An upstream contribution is **not** required in a packet. The requirement is that the control is generic, documented and verified on the targets in use, so someone can share or propose it later without rewriting it.
