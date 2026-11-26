# Rustday

A native desktop workspace for managing boards and items, built with Rust, [Longbridge GPUI Kit](https://github.com/longbridge/gpui-kit) and PostgreSQL. The first end-to-end target: edit a board item, restart the app, and see the saved edit. This repository currently contains the development plan and session runner; the application is the work ahead.

## Run the migration

Use Node 22+, OpenCode v2 with a configured provider, and a Rust toolchain with Cargo and rustfmt. For the second and third packets, supply `RUSTDAY_TEST_DATABASE_URL` for a disposable Rustday PostgreSQL database. The desktop packet also needs a graphical session and the platform's native GPUI build prerequisites. Use a dedicated checkout and development account with no production credentials. Cargo build scripts and tests run with the account's permissions.

```sh
node --test scripts/run.test.mjs
node scripts/run.mjs status
node scripts/run.mjs run 3
```

`run 3` processes up to three pending packets; `run 1` is a single-milestone trial. Each gets a new `opencode run` session, with compaction available inside the session. The worker writes a short handoff; the runner checks it, runs the named Cargo suite and workspace checks, then records a verified checkpoint. The next session reads the handoff and the actual files, not a replay of the previous chat. The runner stops at the end of the queue, on a blocker, after a failed check, or on a command timeout (two hours per invocation).

If the CLI is not on PATH, set `OPENCODE_BIN` to its executable. On Windows with OpenCode Desktop already running, PowerShell can use `$env:OPENCODE_BIN = (Get-Process opencode-cli | Select-Object -First 1 -ExpandProperty Path)`.

**Resume:** inspect the last handoff and terminal error, fix the blocker, and run the same command in the same checkout. Partial edits remain on disk. After an abrupt shutdown, check that the runner and worker have exited before removing `.rustday/lock`. Verification markers are local to this checkout; invalidate the affected marker and later markers if you revert completed work. Review and commit source and public-safe handoffs at milestones; the runner does not publish changes.

See [ROADMAP.md](ROADMAP.md) for acceptance tests and [AGENTS.md](AGENTS.md) for the worker contract. To extend the run, add a small acceptance packet and its test target to the queue in `scripts/run.mjs`.
