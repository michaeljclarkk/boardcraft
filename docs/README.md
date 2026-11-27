# Documentation strategy

Documents answer questions. They are not a second task list.

## Which document answers which question

| Question | Read or update |
|---|---|
| What do I do next, and what counts as done? | [Next task](roadmap.md#next-task) and [delivery order](../ROADMAP.md#delivery-order-owner-direction-2026-11-28). |
| What is still missing or unverified? | [Current gaps](gaps.md). |
| What happened last, and what did it leave? | [Progress log](../ROADMAP.md#progress-log). Newest first, one dated entry per delivery. |
| How do I open a board or recover the environment? | [Rust browser viewer](development.md#browser-viewer) and [development runbook](development.md). Run commands from the repository root. |
| Where does the behaviour live? | [Architecture and owner map](architecture.md). Link to the smallest useful implementation entry point. |
| What stack does the browser client use? | [Browser client stack](web-client-stack.md). Decision, shared controls, migration notes and risks. |
| How is it deployed for clients? | [Deployment shape](deployment.md). Two containers, app on 5078, database private. |
| What do I do when GPUI Kit lacks an interaction? | [Shared GPUI components](gpui-components.md). No-fallback rule, required artifacts and the control register. |
| What is implemented, verified or unknown? | [Workflow register](workflows.md). Track automated checks, desktop observations and unresolved questions separately. |
| What is parked until later? | [Deferred work](deferred.md). Owner decisions with entry criteria; not part of the port. |
| How must a worker behave? | [AGENTS.md](../AGENTS.md). |

## Rules

1. **One owner per question.** A fact lives in one document and the others link to it. Do not restate the delivery order in a feature note, or the workflow register in a scratch note.
2. **Three statuses only:** implemented, verified, unknown. A row may be implemented and unverified; it may never be verified without a named check or observation behind it.
3. **Evidence next to the claim.** Link the test or observation that proves a row. A check that did not run is `unknown`, never `verified`.
4. **One place per delivery.** Do not write a second task list. A delivery gets a dated progress-log entry plus its workflow row; once it outgrows a row, a feature note.
5. **Feature notes on growth, not on creation.** When a workflow outgrows its register row, add `docs/features/<workflow>.md` and link it from the register. Describe the user action, the success and recoverable failure, the rule owner, and how to reproduce the check with synthetic data.
6. **Decisions live beside their behaviour.** Record a consequential design decision in the document that explains that behaviour and date it. Replace obsolete guidance in place; never leave two versions.
7. **Name only what we own.** The internal tool is our own product. Do not name it, its files or its screenshots in a committed document; local details stay in `plan/internal-tool.md` (ignored). Do not name, link or copy a third-party product's assets, code, wording or screenshots either.
8. **Private material stays private.** Captures, records, transcripts, absolute paths and credentials never enter `docs/`. `plan/` is the gitignored notebook.
9. **Keep documents small.** Shorter than the code they describe, and a table row with a link beats a restated paragraph.

## Ownership

`ROADMAP.md`, `docs/architecture.md`, `docs/development.md`, `docs/workflows.md`, `docs/web-client-stack.md`, `docs/gpui-components.md` and this file change at milestone close, with Michael or when the change is the task. A feature session updates its own workflow row, its shared-control register row, and any feature note it was asked to write.
