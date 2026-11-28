---
id: columns
name: Columns and cells
order: 5
percent: 12
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

Estimate 12% credits observed Windows/browser Status work only. Label management, summaries, other column types, source-configured widths, exact native sizing and full parity remain open. Kit exposes checked state for the selected option rather than the source's aria-selected metadata.
