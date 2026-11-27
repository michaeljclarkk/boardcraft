# 54-web-auth-board

On the wasm client, recheck the workflows from packets 26–30: sign in, list authorized boards, open and revisit a selected board, rename an item, recover a rejected save and resume a session.

**Boundary.** Use the existing host routes and access policy. Do not add a parallel API, a second session mechanism or client-side authorization. The wasm shell owns drafts and pending states; the host returns the committed row and current access.

**Controls.** Reuse `boardcraft-ui`. If an interaction is missing on wasm, extend the shared control as in packet 53; do not embed HTML/CSS/JS/React/TSX or keep a web-only control.

**Sign-in and list.** Identifier advances locally; password submission is the only account request. Unknown identifier and wrong password produce the same correctable result. Resolve before reading the list. Render server order with board UUIDs as identity. Distinguish an empty list from an unavailable one and offer retry. Logout waits for acknowledgment and clears protected content.

**Board and name.** Open by UUID from the list or a direct board address. Back and Forward recheck access. A rejected rename returns to the accepted spelling with an explicit retry that reads before resubmitting, or a dismiss. Navigation reads current access again.

**Live acceptance** (pending):

Use Computer Use in the [ordinary viewer](../docs/development.md#browser-viewer). From the signed-in catalog activate **Check session** once and observe the refreshed list. Compare with one session GET followed by one list GET at the host.

- Reject and correct credentials; compare visible names and order with independent reads of UUIDs and grants, including two boards with the same name. Distinguish empty from unavailable. Failed logout retains the list and session; acknowledged logout revokes only the current verifier.
- Open one of the identically named boards; compare its visible items with independently stored rows. Remove membership and reload; require denial, no stale rows and an explicit retry. Check direct entry and Back/Forward against current access.
- Cancel or unchanged input sends no write. Blur or Enter submits a changed draft once. On rejection show the accepted spelling, retain the draft for read-before-resubmit retry, and allow dismissal without a write. Reconnect to check the committed ID, title, revision, order and unchanged peers.
- An independent same-origin tab resolves the same session before reading its board; acknowledged logout clears protected rows and editor state in both tabs.

Check visible focus and masked password entry. A typed password must not leak through the canvas or browser text surfaces (including the IME receiver), during editing or after submit. Keep generated secrets out of argv, URLs, captures and logs. Use GPUI Kit tests for control behavior and retain independent host/store checks. The earlier handoff is historical; record any unobserved live outcome as pending.
