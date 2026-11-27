# Deferred work

Owner decisions parked until an entry condition is met. These are **not** part of the port and must not be started without the owner.

## Move the database to pgrust

**Status:** deferred — owner direction 2026-11-28.

**What:** run boardcraft against [pgrust](https://github.com/malisper/pgrust) instead of PostgreSQL, using the same SQL and wire protocol. Ideally no application code changes.

**Why:** a Rust database with a faster analytical engine, without a second system.

**Why not now:**

- pgrust is v0.4-beta; the project targets production readiness in Q1 2027.
- Licence is AGPL-3.0. Confirm the network-use and distribution obligations before shipping it with boardcraft.
- It targets PostgreSQL 18.x. Our local cluster is 17, so the move needs a dump/restore or an upgrade first.
- Validate migrations, extensions, backup/restore and failure behaviour before trusting it with boards.

**Entry criteria:** pgrust is production-ready, the licence is cleared, and a disposable copy of the boardcraft database passes the full workspace suite, migrations, reconnect, rollback and backup/restore.

**Work when triggered:**

1. Pin the pgrust version or image and record it.
2. Dump the existing database and restore it into pgrust.
3. Run `cargo test --locked --workspace`, the PostgreSQL store tests, the loopback access tests and the desktop reconnect journey.
4. Verify backup/restore and a failed-save recovery on the new engine.
5. Keep PostgreSQL as the rollback target until the engine has run clean for an agreed period.

Do not add pgrust as a dependency, extension or build step before this is triggered.
