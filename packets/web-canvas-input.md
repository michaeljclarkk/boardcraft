# Browser canvas input after the platform bridge removal

Remove the custom web screen-reader adapter, semantic DOM nodes, action forwarding, and patches used only to enable that adapter. Do not remove native accessibility or ordinary GPUI control properties. Preserve secure password entry: neither the visual canvas nor the browser's IME or accessibility text may disclose a typed password. Keep only the minimum independent IME fix if stock GPUI cannot meet this regression. Build via locked public dependencies from a clean checkout.

Launch the [normal viewer via the Rust CLI](../docs/development.md#browser-viewer). With Computer Use, exercise pointer, drag and keyboard input on the canvas. Check focus and password masking, stable identity with duplicate titles, cleared rows after revocation, retained drafts on failure, committed edits after reconnect and logout in both tabs. Retain GPUI Kit control tests and independent checks of session request counts, access and stored results.

Record actual observations separately from build and contract results. Visible masking alone does not establish IME privacy. If a required check cannot be made, leave it pending. Packet 54's earlier driver evidence is historical; do not replace that driver in Rust or any other language.
