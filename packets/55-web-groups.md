# 55-web-groups

Port the grouped board workflows to the wasm client. This re-verifies packets 31–43, 51 and 52 on the new client without changing the host or store contracts.

**Scope.** Grouped selected-board read, group create/edit/order/collapse, item create, move, reorder, selection and selection move, archive and restore of roots and groups, and Trash. Reuse the existing named Rust operations through the host. Do not add web-only mutations or duplicate validation.

**State ownership.** The wasm view owns selection, drafts, pending requests and recovery actions. The store owns accepted order, revisions, receipts and access decisions. A successful command is only shown after the committed result returns; an uncertain result reads before any retry.

**Interaction.** Reuse `boardcraft-ui` for every shared control. If a control is unavailable on wasm, extend it in `boardcraft-ui` with a public API, synthetic example, interaction test, accessibility contract and a note on the missing Kit capability. Do not embed HTML/CSS/JS/React/TSX, a webview, iframe or DOM overlay, and do not write a web-only variant. Keep focus, keyboard path and recovery behaviour equivalent to desktop where the workflow requires it. Record the control in [the register](../docs/gpui-components.md).

**Required gates** (planned, not passing evidence):

- `web_creates_and_edits_group_fields`: duplicate group, cancel, save and reload by ID; assert revisions and unchanged peers.
- `web_orders_and_folds_groups`: menu and drag ordering, single and all folds, hidden selection anchors, reload.
- `web_creates_moves_and_reorders_items`: create with proposed UUID, move by menu, reorder by grip, reload.
- `web_selects_and_moves_items`: range selection across groups, ordered append, conflict review and separate confirm.
- `web_archives_and_trashes_roots`: archive/restore and Trash/restore with receipts and denial cases.

Assert role, name and state through the accessibility tree. Compare accepted IDs, order, revisions, field stamps and receipts with independent reads.
