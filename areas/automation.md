---
id: automation
name: Automation engine
order: 7
percent: 10
code: crates/board/src/automation, crates/desktop/src/automation, crates/web/src/automation
---
# Automation engine

Triggers, conditions, actions, scheduling and retries. Port the rule engine, its editor UI and its desktop/browser/Android behaviour together.

## Workflows

- [x] List automations for a board
- [ ] Create and edit a trigger, condition and action
- [x] Run an automation from a real board change
- [ ] Retry a failed run without double effects
- [ ] Inspect the run result

## Notes

The first slice stores a board automation with the fixed `item_renamed` → `record` rule, lists it with its latest run, and runs it inside the rename transaction only when the accepted title changed. Windows browse created `Audit rename`, renamed Ink to `Ink 2`, and `Last run: Ink 2` appeared when the board was reopened; the independent database read confirmed the run row. Android compiles for `x86_64-linux-android` and renders/lists/creates the same rules, but its device screen was not observed. There is no editor for trigger/condition/action, no run history page, no retry and no browser UI. The run was not refreshed by `Reload board` in this run; reopening the board refreshed it.
