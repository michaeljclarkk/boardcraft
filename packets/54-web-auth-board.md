# 54-web-auth-board

Deliver the first working board workflows in the wasm client: sign in, list authorized boards, open a selected board, rename an item and recover a rejected save. This re-verifies packets 26, 27, 29 and 30 on the new client.

**Boundary.** Use the existing host routes and access policy. Do not add a parallel API, a second session mechanism or client-side authorization. The wasm shell owns drafts and pending states; the host returns the committed row and current access.

**Controls.** Reuse `boardcraft-ui`. If an interaction is missing on wasm, extend the shared control as in packet 53; do not embed HTML/CSS/JS/React/TSX or keep a web-only control.

**Sign-in and list.** Identifier advances locally; password submission is the only account request. Unknown identifier and wrong password produce the same correctable result. Resolve before reading the list. Render server order with board UUIDs as identity. Distinguish an empty list from an unavailable one and offer retry. Logout waits for acknowledgment and clears protected content.

**Board and name.** Open by UUID from the list or a direct board address. Back and Forward recheck access. A rejected rename returns to the accepted spelling with an explicit retry that reads before resubmitting, or a dismiss. Navigation reads current access again.

**Required gates** (planned, not passing evidence):

- `web_signs_in_lists_and_logs_out`: reject and correct credentials; compare visible `(UUID, name)` rows with independent stored order and grants; prove only the current verifier is revoked.
- `web_opens_board_and_rejects_stale_access`: open by UUID, remove membership, reload; require denial with explicit retry and no stale rows.
- `web_renames_and_recovers_rejection`: submit once on blur or Enter; assert the committed row, revision and that a rejected retry reads before resubmitting.
- `web_restores_session_in_a_second_tab`: an independent tab resolves before reading; logout clears both first-origin tabs.

Use the accessibility tree for control identity (role, name, state). Keep generated secrets out of argv, URLs and logs. Close only the owned browser, host and schema.
