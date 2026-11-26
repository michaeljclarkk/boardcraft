# Working on boardcraft

## Start a session

boardcraft is a source-code translation of our internal application. Its C# operations and React/TypeScript (TSX) components define the scope, including our own extensions. A feature offered by the commercial platform alone is not a requirement. Trace each interaction through the source component and its C# operation, then verify the translation with synthetic acceptance cases.

1. Read your own [packets/<packet-id>.md](packets/) for the assigned packet and acceptance criteria; [ROADMAP.md](ROADMAP.md) is an index only. `node scripts/run.mjs status` shows which packets passed in this checkout. Never edit verification markers to advance the queue.
2. Inspect current edits, the previous `handoffs/<packet-id>.json`, and relevant entries in the [owner map](docs/architecture.md) and [workflow register](docs/workflows.md). Work within the assigned packet. Preserve unfinished edits across retries.
3. Use the [development runbook](docs/development.md) for tools and checks. For feature work load `boardcraft-feature` or read its [instructions](.opencode/skills/boardcraft-feature/SKILL.md). A fresh public checkout must build without private material.

## Implement the interaction

**Require 1:1 workflow parity.** Close a workflow only when the same synthetic action produces the expected result on each applicable client, including failure and recovery. Link that evidence in the workflow register; keep untested behavior and observed differences open. Windows and Android are active targets; defer iOS until Android is complete.

The target is the full existing independent product: the workflows it supports, including its own extensions. When its behavior differs from monday.com, follow the approved reference. Preserve observable workflows, permissions, data contracts, UI behavior, failure paths and platform-specific behavior. A milestone is a delivery boundary, not permission to omit a feature. If reference behavior is unclear or a redesign seems necessary, record the gap before proceeding. Use the same synthetic inputs and compare outcomes on both sides.

- Trace the caller to its rule owner before adding code. Use a named Rust operation for a business decision; reuse it for UI actions, imports and jobs that mean the same thing. Extract shared logic when there is a real second caller. Similar values with different meanings may change independently.
- Use stable IDs, explicit inputs and typed outcomes. A label, workspace ID or hidden button is not an access grant. Keep database reads bounded and ordered. Persist related writes in one transaction; only report success after commit.
- Start with Longbridge GPUI Kit controls. For a custom control, record the missing interaction in the feature entry. Keep drafts and focus with the view; use the accepted Rust result for saved state. Keep database work off the UI thread. Handle cancellation, conflicts and retries without losing drafts.
- Distinguish validation, missing records and unexpected database faults. Give the user a recovery action. Log an unexpected failure once at its owning boundary, with operation and identifiers but no credentials or user payloads. Required follow-up work must survive restart; a UI refresh is not durable delivery.

## Scope and verification

Run unattended by default. If a check needs the owner's input, clipboard handoff or supervision, use an isolated fixture when available. Otherwise skip that check, record what remains unverified, and continue without waiting for the owner. An attended run authorizes only that run.

On Windows, read configured credentials from the process environment. Keep interactive sign-in sessions in memory; do not use Windows Credential Manager or persist desktop passwords or tokens.

Translate C# business rules into Rust and React interactions into Longbridge GPUI Kit views. For example, follow a rename from the TSX draft through the C# save result so the native editor preserves focus, accepted values and recovery behavior. Do not use monday.com code or assets, including logos, icons, fonts, screenshots or templates.

Use newly written boardcraft code, synthetic fixtures and a dedicated test database. Compare with approved reference behavior where available; record unknowns rather than guessing. Keep private names, paths, screenshots, records, transcripts and credentials out of public files, including skills and handoffs. Do not publish, commit, push, select a license, deploy, or change runner gates during a worker packet.

Run focused success and failure tests; leave the full packet suites to the runner once at handoff. For persistence, reconnect and read back accepted state. For interaction changes, exercise the control in a running GPUI window: compilation does not verify keyboard input, focus or recovery. Use the `gpui-kit` coding guides and `gpui-kit-design-guides` for native UI work. Identify any checks that did not run and why.

## Close a packet

Plan sizeable port commits around **10,000–20,000 changed lines**, splitting earlier at a working boundary when appropriate. Small fixes stay small. Above 20,000 lines, split before committing. Inspect diff statistics before opening large patches; use one active build at a time. See [development](docs/development.md#change-size).

Update the affected workflow row and its code/test links. Do not edit the owner map, `docs/architecture.md`, `docs/development.md`, `docs/README.md` or [ROADMAP.md](ROADMAP.md) during a packet; raise the change to the coordinator and let it land at milestone close. Avoid a second competing task list.

Write `handoffs/<packet-id>.json` with `status` (`ready` or `blocked`), `summary`, `evidence` (array of actual checks and outcomes), and `next` (one concrete action). All except `evidence` are strings. A blocker gets a handoff too. Only the runner marks a packet verified, after its checks pass.

## Model roles

| Role | Model | Use for |
|---|---|---|
| Coordinator | Astra 6.2, extra high | Choosing the next workflow, writing its packet file, reviewing handoffs and diffs, committing and pushing. |
| Implementation | Sol 6.1, extra high | Rust, SQL, GPUI Kit views and acceptance tests. |
| Research | DeepSeek V4 Flash 1017 | Read-only sweeps of the reference source, probe analysis and locating rule owners. No workspace edits. |

Research agents return findings; they never write code. Exact CLI identifiers belong in the runner's per-packet `model` field (`openai/gpt-6.2-astra#high` is the current form); the owner sets the effort suffix. Do not invent a model identifier that the CLI has not confirmed.

## Continuous delivery on main (coordinator)

Work directly on `main` until the roadmap's 1:1 release gate is satisfied. Dispatch one fresh worker per workflow-grain packet, not per gesture. After the runner verifies its handoff, review the diff, evidence and staged files for scope and private material, and check that no process document outside the workflow row changed. Commit the tested change and push to `origin/main` without force. Confirm the remote SHA, then report the commit link, what passed and the next packet. Start that packet without waiting for a reply.

An empty runner queue is not completion: choose the next uncovered source workflow from the register, write its packet file and acceptance case, add its runner gate, then dispatch it. Adding a gate for a new workflow is the coordinator's job; extending gates mid-packet is not. Fix ordinary failures and retry. Interrupt the owner between pushes only for a blocker that needs their access, decision or direct observation.
