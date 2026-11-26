# Documentation map

| Question | Read or update |
|---|---|
| What do I do next and what counts as done? | [Roadmap](../ROADMAP.md). One ordered queue; do not restate the schedule in feature notes. |
| How do I build, start and recover the environment? | [Development](development.md). Keep commands runnable from the repository root. |
| Where does the behavior live? | [Architecture and owner map](architecture.md). Link to the smallest useful implementation entry point. |
| What is implemented, verified or unknown? | [Workflow register](workflows.md). Track automated checks, desktop observations and unresolved questions separately. |
| What did the last session leave on disk? | `handoffs/<packet-id>.json`. Record evidence and one next action; do not paste the transcript. |

When a workflow outgrows its row, add `docs/features/<workflow>.md` and link it from the register. Describe the user action, success and recoverable failure, rule owner, and how to reproduce the check with synthetic data. Keep consequential design decisions beside the behavior they explain. Replace obsolete guidance in place.
