---
id: automation
name: Automation engine
order: 7
percent: 55
code: crates/board/src/automation, crates/desktop/src/automation, crates/web/src/automation, android/src/automation
---
# Automation engine

Triggers, conditions, actions, scheduling and retries. Port the rule engine, its editor UI and its desktop/browser/Android behaviour together.

## Workflows

- [x] List, search and page automation rules, including archived rules
- [x] Create and edit a trigger, condition and action, including conditional groups
- [x] Enable, disable, archive, restore and duplicate a rule
- [x] Run an automation from item creation, rename, status and group changes
- [x] Retry or replay an accepted transition without double effects
- [x] Inspect the run result and its stored snapshots
- [ ] Browser Manage/Run history/wizard panels (state exists; render and board integration remain)
- [ ] Button-click, conversation, mention and reaction triggers
- [ ] People and date actions on real board columns
- [ ] Android connected run observed on a device

## Notes

The engine now stores the full closed definition: 8 trigger kinds, 4 condition operators, 9 action kinds, AND/OR modes, conditional action groups, notification recipients and date bases. Migration 19 converts the first fixed rename rule into a `column_changed(Name)` rule with a creator notification, preserving its run history. The store owns catalog, paginated rules, revision conflicts, idempotent create, the 250-rule limit, enable/disable and archive/restore.

The executor runs inside the caller's mutation transaction. It matches saved triggers and conditions, executes status changes, group moves and notifications, enqueues the resulting column changes for downstream rules, stores trigger/action snapshots and uses a durable idempotency key so an accepted transition cannot apply twice. Host routes cover catalog, rules and history; the host hooks item creation, name, status, single-group moves and selection moves. Board tests cover the engine, replay safety, condition failure and migration; the loopback test covers owner/member routes, execution and the inbox.

Windows has the complete Manage/Run history panel and create/edit/duplicate wizard in `crates/desktop/src/automation.rs`, opened from **Automate** and Board tools. The Android screen in `android/src/automation.rs` compiles for `x86_64-linux-android` and replaces the old inline list/create. The browser state/draft model exists in `crates/web/src/automation.rs`, but it has no render or board integration yet, so it contributes no verified UI.

Not ported: button columns and their click trigger, conversation/mention/reaction events (the Conversations area is not ported), people/date actions (those column types are not ported), dependency tracking when a referenced column or group is removed, and a device-observed Android screen. Desktop was compiled and its GPUI panel/tab control test passed; it was not observed in a live signed-in window in this slice.
