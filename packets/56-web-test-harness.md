# 56-web-test-harness

Replace the browser acceptance driver with one that targets the wasm client's accessibility tree. This packet creates the evidence mechanism that lets packet 57 delete the JavaScript preview.

**Remove the assumption of DOM controls.** A GPUI canvas has no queryable HTML controls. Drive the application through the accessibility tree: role, accessible name, state and supported action. Assert accepted application state through independent host and PostgreSQL reads, never through the canvas.

**Controls must expose semantics.** Every interactive element exercises a role, name, value/state and keyboard path. If a shared control is anonymous, fix it in `crates/boardcraft-ui` and document its accessibility contract in [the register](../docs/gpui-components.md); prove the fix on desktop and wasm. Do not add test-only hooks, hidden DOM proxies or a scripted back door.

**Do not port the DOM assertions.** Write new accessibility tests. Copying selector-based checks would encode the deleted preview. Keep failure injection, independent database checks and owned browser/profile cleanup; packet 57 deletes the old harness with the preview.

**Required gates** (planned, not passing evidence):

- `web_a11y_control_contract`: every control used by packets 54 and 55 has a stable role and name, and actions are invocable.
- `web_keyboard_and_focus_paths`: tab order, Escape, Enter and focus restoration match the workflow requirement.
- `web_accessibility_tree_is_stable`: IDs remain stable across renders and reorders; no duplicated or missing nodes.
- `web_browser_rows_are_reverified`: each browser row in the register has a named wasm check and independent reads.

Record which rows are covered and which remain unknown. Do not delete the JavaScript harness in this packet; packet 57 removes it completely with the rest of the preview.
