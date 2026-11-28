---
id: columns
name: Columns and cells
order: 5
percent: 15
code: crates/board/src/columns, crates/mobile-edit-host/src/columns.rs, crates/mobile-edit-host/src/web_session.rs, crates/mobile-edit-client/src/status.rs, crates/boardcraft-ui/src, crates/desktop/src/connected/main_table/status.rs, crates/web/src/groups, crates/web/src/session.rs, android/src/navigation/status.rs
---
# Columns and cells

Every column type: setup, defaults, validation, editors, display, formulas, linked items and file cells. The same rule serves direct edits, batch writes and imports.

## Workflows

- [x] Item name cell
- [ ] Status, date, people, number and text columns
- [ ] Link, location and country columns
- [ ] Formula and derived columns
- [ ] File cells and previews
- [ ] Validate, clear and reload every type

## Notes

Status now exists in the Rust browser, with the existing authorized rules and independent cell revisions. Windows/browser have source two-column palettes, selected/empty states, always-visible Clear value, no-op dismissal, focus restoration and in-cell saving feedback. A live browser conflict reread retained the newer Done value without replaying the stale Working on it choice; clearing then showed an empty cell. Cross-client Windows reads observed browser saves. Focused host checks cover signed browser access, set/replace/clear, conflict, Viewer denial and revocation. Android picker/recovery changes build and run to sign-in in the preserved companion; connected Status observation remains pending. See the workflow register for final runtime evidence and limits.

Selection now exposes Change label… → Status → Choose label across Windows, Rust/WASM and Android. It reuses per-cell revisions and write authorization, includes hidden selected IDs, skips unchanged values, retains successful writes and names failed items, and rereads uncertain results without replay. Browser retry targets the failed item; Windows requires choosing a label again after review. Computer Use verified browser partial success/retry/Clear and Windows two-conflict recovery, deliberate retry, Clear and Escape. Cross-client reload confirmed persistence. A live Windows accessibility-ID collision was fixed and covered by a two-conflict regression. Android builds and runs to sign-in; its connected bulk journey remains unverified.

Estimate 15% credits observed Windows/browser Status and selection plus browser label management. The inline Owner editor, per-column persistence, configured default metadata, autosave/new labels and local draft recovery are implemented across Rust clients. Browser name/default-color/new-label and failed-save reload/retry were observed. Final browser Custom hex save, equal odd tracks and Escape focus were observed. Native label interactions and Android connected touch/lifecycle remain unverified; no native percentage increase. Custom chooser/palette dismissal still differs from the source. Summaries, other column types, configured widths and exact native sizing remain open. Kit exposes checked state rather than the source's aria-selected metadata. Source review confirms null displays the configured default name/color; Clear value and that default choice both write null. See the newest workflow entry.
