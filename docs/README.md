# Documentation strategy

Documents answer questions. They are not a second task list.

## Which document answers which question

| Question | Read or update |
|---|---|
| What do I do next, and what counts as done? | [Roadmap index](../ROADMAP.md) plus your own file in [packets](../packets/). Read only your own packet. |
| How do I open a board, run a packet or recover the environment? | [Rust browser viewer](development.md#browser-viewer), [queue commands](../README.md#run-the-queue) and [development runbook](development.md). Run commands from the repository root. |
| Where does the behaviour live? | [Architecture and owner map](architecture.md). Link to the smallest useful implementation entry point. |
| What stack does the browser client use? | [Browser client stack](web-client-stack.md). Decision, shared controls, migration packets and risks. |
| What do I do when GPUI Kit lacks an interaction? | [Shared GPUI components](gpui-components.md). No-fallback rule, required artifacts and the control register. |
| What is implemented, verified or unknown? | [Workflow register](workflows.md). Track automated checks, desktop observations and unresolved questions separately. |
| What did the last session leave on disk? | `handoffs/<packet-id>.json`. Evidence and one next action; never a transcript. |
| How must a worker behave? | [AGENTS.md](../AGENTS.md). |

## Rules

1. **One owner per question.** A fact lives in one document and the others link to it. Do not restate the queue in a feature note, or the workflow register in a packet.
2. **Three statuses only:** implemented, verified, unknown. A row may be implemented and unverified; it may never be verified without a named check or observation behind it.
3. **Evidence next to the claim.** Link the test, packet or observation that proves a row. A check that did not run is `unknown`, never `verified`.
4. **No document per packet.** A packet is already a file. Prose about a packet belongs in its handoff (`evidence`, `next`) or, once it outgrows a row, in a feature note.
5. **Feature notes on growth, not on creation.** When a workflow outgrows its register row, add `docs/features/<workflow>.md` and link it from the register. Describe the user action, the success and recoverable failure, the rule owner, and how to reproduce the check with synthetic data.
6. **Decisions live beside their behaviour.** Record a consequential design decision in the document that explains that behaviour and date it. Replace obsolete guidance in place; never leave two versions.
7. **Name nothing we do not own.** Documents follow [independence](../AGENTS.md#independence): no reference-product names, links, screenshots or copied wording, in any document or handoff.
8. **Private material stays private.** Captures, records, transcripts, absolute paths and credentials never enter `docs/`, `packets/` or `handoffs/`. `plan/` is the gitignored notebook.
9. **Keep documents small.** Shorter than the code they describe, and a table row with a link beats a restated paragraph.

## Ownership

`ROADMAP.md`, `docs/architecture.md`, `docs/development.md`, `docs/workflows.md`, `docs/web-client-stack.md`, `docs/gpui-components.md` and this file change at milestone close, through the coordinator. A worker updates only its own workflow row, its handoff evidence, its shared-control register row, and a feature note it was asked to write.
