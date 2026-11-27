# 56-web-test-harness

Use the [ordinary Rust/WASM viewer](../docs/development.md#browser-viewer) to recheck the browser workflow rows from packets 53–55. The packet ID is historical; it does not call for building a harness.

- Use Computer Use to tab to a control, activate it, cancel with Escape and observe where focus returns. At the host, check that a session check resolves once, a save commits one revision and cancel sends no mutation.
- Reorder two same-title rows, select and edit one, then reopen. Compare the visible selection and order with the requested UUID and fresh PostgreSQL rows. After removal, the stale row must no longer be visible or actionable.
- Keep GPUI Kit tests for control behavior and independent host/store tests for identity, permissions and rollback. Neither a build nor a database read verifies a visible result.

Record the action and visible outcome for each exercised row. Leave the packet pending if an essential Computer Use observation is missing. Do not add a browser or GUI driver, OCR, CDP, DOM mirror or substitute in any language.
