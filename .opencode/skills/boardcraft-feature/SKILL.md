---
name: boardcraft-feature
description: Build or review a boardcraft workflow so it ships end to end with both workflow parity and UI parity against the internal tool.
---

Start with the user action. What does the user click, what record changes, what do they see, and what happens when the save fails? Finish that journey in the running app before widening scope.

## Order of work

1. Run the app and the same screen in the internal tool side by side.
2. Build the smallest vertical slice: control → named Rust operation → PostgreSQL commit → accepted result back in the view.
3. Exercise it in the running app, restart, and check the saved state.
4. Match the internal tool's UI, then record any remaining difference.

Do not batch a large packet or wait for a test gate to declare the work real.

## Parities

**Workflow parity:** same action, inputs, controls, result, failure and recovery as the internal tool.

**UI parity:** same screen structure, control choice, labels and order, density, emphasis, empty/loading/saving/error states, keyboard and focus behaviour, pointer and touch behaviour, and visible result. Compare screenshots at the same window size and fixture.

A workflow with an open UI difference stays open in [docs/workflows.md](../../../docs/workflows.md). Passing tests do not close it.

## Rules

- One named Rust operation owns the business rule. UI actions, imports and jobs call it.
- Stable IDs, explicit inputs, typed outcomes. A label or position is not an identity.
- One transaction for related writes; report success after commit.
- Keep drafts on failure; give the user retry, reload or recovery.
- Keep database work off the UI thread.
- Start with stock GPUI Kit controls. If one is missing, add the smallest working control in `boardcraft-ui`, use it, then improve it. Do not block the screen on docs, examples or accessibility gates.
- Port the internal tool's behaviour and translate its logic into Rust; it is our product. Its C#/React/XAML/CSS is reference only — no JavaScript, TypeScript, XAML or CSS goes into boardcraft. Keep the stack 100% Rust.

## Verification

- Focused Rust tests at the boundary that can break: store for persistence, host for access, a running control for input and focus.
- Use the running desktop app and Computer Use for native windows; Chrome with the `chrome-devtools` MCP for the browser.
- No custom browser driver, OCR, accessibility mirror, DOM mirror or substitute harness. If a visual check cannot run, say what you could not see.
- Report the actions you performed and what you saw, not just a green build.
