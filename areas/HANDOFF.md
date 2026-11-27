# Handoff template

Copy this, fill the `<...>` fields and paste it into a fresh agent session. Do not edit the rules per handoff; change this file only with the owner.

---

**Handoff — Rustday / boardcraft port**

Repo: `C:\Users\Michael\source\repos\Rustday`

Read first: `AGENTS.md`, `README.md` → Port map, `areas/README.md`, then `areas/<area-id>.md`.

**Area:** `<area-name>` (`areas/<area-id>.md`)

**Screen:** `<one screen or action>`

**Reference fixture:** `<same board, window size and data in both apps>`

**Only job:** make boardcraft 1:1 with the internal tool, in Rust only. Write Rust, run it, verify the UI with Computer Use and GPUI Kit tests.

**This session:**

1. Open the internal tool on the reference fixture, and boardcraft beside it.
2. Port the smallest real workflow end to end: Rust rule → persistence → UI → desktop/browser/Android where it applies.
3. Once the contract is stable, run exactly two **UI** and **cross-platform** subagents concurrently. Call the `subagent` tool twice with `background: true` and `agent: general`:
   - UI: “Implement `<screen>` in `<area>`; stay inside the area's code homes; do not commit, add crates or write docs.”
   - Cross-platform: “Port the same screen to Android and the shared client; stay inside the area's code homes; do not commit, add crates or write docs.”
4. Work the contract and integration while they run. When they return, review their changes, run both apps and reconcile the result.
5. Run both apps. Compare workflow and UI. Fix the differences, or record them in `docs/workflows.md`.
6. Update `percent` in `areas/<area-id>.md` honestly — verified behaviour only, never code, tests or scaffolding.
7. Commit and push: `port(<area-id>): <screen>`. The progress-log entry must name both subagent session IDs.

If the subagent tool is unavailable or a call fails, say so in the progress-log entry. Do not silently skip the parallel step.

**Commit only:** port code under the area's `code` homes, `areas/<area-id>.md`, `docs/workflows.md`, and the generated `README.md` / `docs/port-status.svg`. The pre-commit guard rejects everything else. If `main` moved, `git pull --rebase`; never force-push.

**Never:** harnesses, drivers, OCR, CDP, accessibility mirrors, orchestrators, queues, packets, runners, handoffs, new crates or dependencies, auth or infrastructure the screen does not use, unrelated refactors. 100% Rust. No `.mjs`.

**Reference:** local-only details are in `plan/internal-tool.md`. Never commit `plan/` or name the tool in files, commits, branches or pull requests.

**Commands:**

```sh
cargo run --locked --manifest-path tools/Cargo.toml -- viewer
cargo run --locked --manifest-path tools/Cargo.toml -- viewer --for-seconds 180
cargo run --locked --manifest-path tools/Cargo.toml -- dev -- cargo build --locked -p boardcraft-desktop
cargo test --locked --workspace
```

If `cargo` is not on PATH, use `E:\RustdayDev\cargo\bin\cargo.exe` with `CARGO_HOME=E:\RustdayDev\cargo` and `RUSTUP_HOME=E:\RustdayDev\rustup`.

**Stop** when the screen matches in the running app and the area row is updated. Do not start the next area; the owner decides.
