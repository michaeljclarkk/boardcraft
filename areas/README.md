# Areas

One file per port area. These are the structural units for handoffs and progress.

**This list is closed.** The approved work is exactly these areas. Do not add an area, feature, subsystem, harness or process outside them. If a task does not fit one of these files, it is out of scope — ask the owner, do not invent work.

Each `<area>.md` has frontmatter with `id`, `name`, `order`, `percent` and `code`, then the scope, a workflow checklist and notes.

Rules:

- A handoff names one area. Port its Rust rules, its UI and its desktop/browser/Android behaviour together.
- Once the area's contract is stable, run UI and cross-platform work as two concurrent OpenCode subagents. The main agent owns the contract, integrates and verifies.
- Work one screen at a time inside the area.
- Change `percent` only after verifying behaviour in the running app. 0 = not started; 100 = every workflow matches on every active client, behaviour and UI.
- Commit and push after each verified screen as `port(<area>): <screen>`. Only port code, its area row and the generated status files may be staged; the pre-commit guard rejects everything else.
- Never count code written, tests or scaffolding.
- Put new code under the `code` path listed in the area. Create the module when the first real workflow lands, not before.
- Regenerate the README table and chart with `cargo run --locked --manifest-path tools/Cargo.toml -- port-status`. The pre-commit hook and CI do it automatically.

The table and chart are generated. Do not edit them by hand.
