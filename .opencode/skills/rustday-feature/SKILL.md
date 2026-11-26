---
name: rustday-feature
description: Implement or review a Rustday workflow across Rust, PostgreSQL and GPUI, including state ownership, failure recovery and milestone evidence.
---

Start with a user action. What record does it affect, what must stay true, and what will the user see if saving fails?

- **One rule owner.** Trace the interaction from the Kit control to a named Rust operation and back. A keyboard shortcut, context menu and import performing the same action should call that operation. Use stable record IDs; titles and row positions can change.
- **One commit point.** Keep related PostgreSQL writes in a transaction. Report the accepted value after commit. If an edit also schedules required work, persist that intent with the edit and make retries safe. Only add this machinery to a feature that needs it.
- **A recoverable editor.** Run I/O away from the GPUI event thread. Distinguish an editable draft from a pending request and a saved value. On failure, keep the draft and give the user a way to retry or reload. Show a stale edit as a conflict rather than silently overwriting another user's change.
- **Proportionate reads.** Fetch the rows and fields the view needs in a stable order. Check for a query per row before reaching for a cache. Prefer a stock Kit control unless a specific interaction requires a custom one.

Test at the boundary where a mistake would show up. For a rename, create two items with the same title, edit one, reconnect, and check both IDs and their order. Interrupt a save and verify the stored value and the draft independently. In the handoff, identify the rule owner, the check that passed, and anything that still needs to be exercised in the running app.
