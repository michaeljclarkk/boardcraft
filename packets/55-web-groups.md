# 55-web-groups

Port the grouped board workflows to the wasm client and recheck the outcomes from packets 31–43, 51 and 52 without changing the host or store contracts.

**Scope.** Grouped selected-board read, group create/edit/order/collapse, item create, move, reorder, selection and selection move, archive and restore of roots and groups, and Trash. Reuse the existing named Rust operations through the host. Do not add web-only mutations or duplicate validation.

**State ownership.** The wasm view owns selection, drafts, pending requests and recovery actions. The host enforces current access; the store owns accepted order, revisions and receipts. Show success only after the committed result returns; read before retrying an uncertain result.

**Interaction.** Reuse `boardcraft-ui` for every shared control. If a control is unavailable on wasm, extend it in `boardcraft-ui` with a public API, synthetic example, interaction test, accessibility contract and a note on the missing Kit capability. Do not embed HTML/CSS/JS/React/TSX, a webview, iframe or DOM overlay, and do not write a web-only variant. Keep focus, keyboard path and recovery behaviour equivalent to desktop where the workflow requires it. Record the control in [the register](../docs/gpui-components.md).

**Live acceptance** (pending): use Computer Use in the [ordinary viewer](../docs/development.md#browser-viewer); retain GPUI Kit tests for control behavior.

**Temporary viewer identity (owner request).** While porting, open Review as a fixed local actor on every visit, without credentials, a sign-in screen or a one-time link. Use the same actor for saves. Revisit authentication once the port is complete.

- Duplicate a group, cancel, save and reload by ID; check revisions and unchanged peers.
- Reorder by menu and drag; check single and all folds and hidden selection anchors after reload.
- Create an item with its proposed UUID, move by menu, reorder by grip and reload.
- Select a range across groups, append in order, review a conflict and confirm separately.
- Archive/restore and Trash/restore; check receipts and denial cases.

Check visible focus, selection, folds and recovery state. With duplicate titles, act on one row and verify the requested UUID, the affected row and the unchanged peer after reorder and reload. Removed rows cannot still be selected. Compare request counts, accepted IDs, order, revisions, field stamps and receipts through the host/store checks. If a required live path cannot be observed, leave the packet pending.
