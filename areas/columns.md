---
id: columns
name: Columns and cells
order: 5
percent: 5
code: crates/board/src/columns, crates/boardcraft-ui/src/columns
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

Only the name cell exists. Create `crates/board/src/columns` and the matching UI module as the first real column lands.
