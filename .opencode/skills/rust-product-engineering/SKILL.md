---
name: rust-product-engineering
description: 'Use when designing, building, testing or reviewing a Rust product or service, choosing a workspace, crate or module layout, deciding where shared Rust code belongs, or when asked to apply product-engineering practice to Rust. Guides agents to layer a workspace in one direction, keep a directory tree a newcomer can navigate, give each rule and contract one owner instead of a second copy, and write idiomatic Rust with explicit state ownership, typed outcomes and recoverable failure. Use during implementation, not only review.'
---

# Rust Product Engineering

## Priority Order

Four demands, in this order when they conflict:

1. **A finished workflow.** A user can do the thing in the running app, including loading, empty, saving, conflict and recovery states, and durable follow-up that survives a crash.
2. **Shared Rust code.** A rule, type, contract or control is written once. A second copy is a defect with a delay fuse.
3. **Architecture that cannot rot.** One direction of dependency between crates and one owner per business rule.
4. **Idiomatic Rust.** Types that make invalid states unrepresentable, no panics on runtime paths, errors that say what happened.

Ship the workflow first; keep the other three intact by keeping scope small. Never fork a rule or duplicate state for speed — a quick module that forks a rule is a rewrite with extra steps.

## The Maintainer Test

Leave one traceable path per substantive feature:

**UI event -> typed request -> thin host handler -> named Rust operation -> one database transaction -> durable follow-up -> accepted result back to the view.**

Another engineer should find the rule with one search of its name, and read the change without learning an invented mini-framework. Concrete functions and types with direct calls are the default. An abstraction earns its place through a real second caller or an external boundary.

If the feature needs no crate, module, job, cache or trait, add none for symmetry. Keep domain rules out of HTTP handlers and view widgets, but let straightforward reads stay straightforward.

## 1. Workspace Architecture

### Layer the workspace

A crate may depend only on a strictly lower layer, dev-dependencies included. The layer table is the contract; the dependency graph is the evidence.

| Layer | Crates | Owns | May depend on |
|---|---|---|---|
| L0 domain | `boardcraft-board`, `boardcraft-account` | Board rules, the PostgreSQL store and its migrations; sessions, credentials and their schema | No workspace crate, no UI toolkit, and not on each other |
| L0 UI library | `boardcraft-ui` | Reusable, domain-free GPUI controls | No workspace crate; `gpui-kit` only; must build native and `wasm32-unknown-unknown` |
| L1 contract | `boardcraft-mobile-edit-client` | Wire request and response types, and the HTTP client every native and browser client uses | Domain identity types only, never the store |
| L2 host | `boardcraft-mobile-edit-host` | The loopback host, access policy, routing and browser session | L0, L1 |
| L3 app | `boardcraft-desktop`, `android/`, `crates/web` | Windows, Android and wasm shells, drafts and presentation | Anything below |

Rules that follow from the table:

- A domain crate never depends on a client, a host, a UI toolkit or an app. If a store needs a display routine, the display routine lives in the app.
- UI toolkit dependencies (`gpui-kit` and friends) are allowed only in the UI library and app crates. A domain crate that needs to show something is the wrong design.
- The Android workspace is a separate `Cargo.toml` but takes the same crates by path. It never re-implements a rule or a wire type to avoid the dependency; there is no Android-only business rule.
- Test-support crates are dev-dependencies only, in every layer.
- Sideways dependencies inside one layer need an explicitly recorded order, or the two crates merge. Prefer merging: two crates that always change together are one crate with two files.

### Register and enforce the graph

- Every workspace crate is registered in one table with its layer. An unregistered crate fails the check, so a new crate cannot quietly land anywhere it likes.
- Enforce it with a workspace tool that reads `cargo metadata --format-version 1 --no-deps` and fails on upward, sideways, cyclic or unregistered dependencies. Keep the rule engine separate from the metadata parser so the rules are unit-testable.
- The check runs with the other gates, in the same command that runs formatting, clippy and tests, and stops at the first failure.
- Moving a crate between layers or adding one is a recorded architecture decision in `docs/architecture.md`, not a side effect of a feature change. Where the tree and the document disagree, the document is wrong until it is updated.

### Crate boundary rules

- A crate earns its place by owning a rule set or an external boundary: a database schema, an HTTP surface, a UI toolkit, a platform, a wire format. "Shared code" is not a boundary.
- Never create a crate named after the product (`boardcraft-core`, `boardcraft-common`). It becomes the crate everything imports and nothing owns.
- Binaries are thin: parse arguments, configure, call one library entry point, report the outcome. No business rule ever lives in `main.rs` or a binary-only test helper.
- Libraries declare `[[test]]` and `[[example]]` with `required-features` rather than spawning ad-hoc binaries.
- New crate proposals answer three questions: what does it own, what layer is it, and which existing crate would have to depend on it? If the third answer is one caller, do not create it.

### Cargo policy

- `[workspace.package]` holds every field the crates share (edition, version, `rust-version`); crates inherit with `field.workspace = true`. Do not select a licence here without the owner.
- `[workspace.dependencies]` holds every third-party version and every internal path. Crates write `foo.workspace = true`. One version bump, one place; two GPUI revisions in one graph is a bug.
- `[workspace.lints]` is where the standard lives: `unsafe_code = "forbid"`, `wildcard_imports = "deny"`, `dbg_macro` and `todo` as warnings that must be cleared before handoff, and complexity limits that mean something. Each crate opts in with `[lints] workspace = true`.
- `Cargo.lock` is committed and every command uses `--locked`.
- External shared libraries taken by git get an exact `rev`. Never a branch or a tag that can move.
- Profiles belong here so one crate cannot argue with another: keep debug usable for GPUI, dependencies optimised, release with thin LTO. Do not copy a profile into a member.
- `cargo tree -d` stays clean. A duplicate is either removed or justified in the handoff.

## 2. The Directory Tree

### Shape

```text
Cargo.toml              members, [workspace.package], [workspace.dependencies], [workspace.lints], profiles
Cargo.lock              committed
rust-toolchain.toml     one pinned channel with rustfmt and clippy
clippy.toml             complexity thresholds, test exemptions, disallowed types
AGENTS.md               the house rules
docs/                   architecture.md (owner map and layers), workflows.md, development.md
crates/
  board/
    Cargo.toml
    migrations/
      0001_boards.sql
      0002_item_revision.sql
    src/
      lib.rs            declarations and re-exports only
      ids.rs            BoardId, GroupId, ItemId newtypes
      error.rs          BoardError and its Display
      model.rs          entities and invariants; no I/O
      store.rs          the store module root
      store/
        items.rs        one file per command family
        groups.rs
        lifecycle.rs
    tests/
      items.rs          integration tests against the real database
  account/              the same shape as board
  boardcraft-ui/
    src/
      lib.rs
      controls/
        heading_drag.rs
    examples/
      heading_drag.rs   one runnable example per shared control
  mobile-edit-client/   wire types and client
  mobile-edit-host/     host, access policy, routing
  desktop/              Windows app
  web/                  wasm shell
apps/                   target home for binaries and command-line tools
xtask/                  cargo xtask ci | layers | wasm
android/                separate workspace, same crates by path
```

`crates/` holds libraries; `apps/` holds binaries. The desktop and web packages currently sit in `crates/`; moving them is a recorded architecture decision, not a packet side effect. Migrations live with the crate that owns the tables they create.

### Module rules

- `lib.rs` is a table of contents: module declarations and re-exports. Logic lives in a named module so the public surface and the implementation can change for different reasons.
- Use `foo.rs` beside `foo/`. There is one way to spell a tree; `mod.rs` is not it.
- One concept per file, named after the concept (`items.rs`, `heading_drag.rs`), not after its role in a pattern.
- Group by responsibility, not by kind. A parallel `models/`, `services/`, `handlers/` split scatters the rule that must be read together.
- Default visibility is private, then `pub(crate)`. `pub` is a promise to every caller in the workspace. Re-export deliberately; keep module paths private so `store::items::rename` never becomes API by accident.
- A store module gets one file per command family, not one file per table. The validation, the transaction and the concurrency check live beside the statement they protect.
- Migrations are numbered, append-only, owned by their crate, and never edited after they have run anywhere.
- Unit tests sit in the module beside the rule. Integration tests go in `tests/` when they need a database, a host or a window. A UI acceptance test belongs to the app crate.
- Examples in `examples/` are for shared controls, seeds and reproductions. A third ad-hoc binary is a missing example or a missing test.

### Smells

| Smell | Why it is wrong | Instead |
|---|---|---|
| `utils.rs`, `helpers.rs`, `common.rs`, `misc.rs` | A file with no owner collects unrelated code and never shrinks | Name the concept, put it with its owner |
| `manager.rs`, `service.rs`, `handler.rs` | Names a pattern, not a rule | `rename_item`, `archive_page` |
| `models.rs` holding every entity | Everything depends on everything | `ids.rs`, `model.rs` per concept; split when it grows |
| `lib.rs` with logic | Surface and implementation change for different reasons | Re-exports only |
| `mod.rs` | Two spellings of one tree | `foo.rs` beside `foo/` |
| Duplicated DTOs in client and host | Two contracts that drift silently | One owner in the contract crate |
| `boardcraft-core` | Everything imports it, nothing owns it | Name the responsibility |
| Business logic in `src/bin/` | An untested second path | Thin binary over a library entry |
| A file with four unrelated owners | The next change touches a stranger's code | Split by owner |

### Naming

- Crates are `<product>-<domain noun>`; the layer table refers to the noun alone (`board`, `account`, `ui`, `host`).
- Types say what they are: `ItemId`, `BoardRevision`, `RenameItem`, `RenameItemError`. Not `DbItem`, `HttpBoard` or `ItemData`.
- Operations are `verb_noun` (`rename_item_at_revision`, `archive_page`, `reorder_groups`) and keep the name the UI action, menu, shortcut and import all call.
- A module named after the product (`board::board`) is a smell; the crate already said it.
- Do not name a type after a technology unless the technology is the subject. `PostgresStore` is fine; `ItemPg` is not.

## 3. Shared Rust Code

Reuse is the second priority for a reason: the cheapest code to maintain is the copy that does not exist.

### Reuse before you add

- Search before writing a rule, type, limit, control, query or wire field. Search for the behaviour under other names and literals, not just for the identifier you were about to create.
- Read the owner map in `docs/architecture.md` and the nearest crate's `lib.rs` before choosing a home.
- A second caller is the signal, not the licence to copy. When the second caller appears, switch it to the existing owner in the same change.

### Where shared code lands

| Shared by | Home |
|---|---|
| Callers inside one crate | A private function in that crate |
| Two crates that both mean the business rule | The lower domain crate that owns it (`board`, `account`) |
| A client and a host exchanging data | The contract crate (`mobile-edit-client`) |
| Two clients showing the same interaction | The shared UI library (`boardcraft-ui`) |
| Two products, genuinely the same problem | An external shared crate at an exact `rev`, with its own tests and its own release |
| Nothing yet | Nowhere. Keep it local until the second caller exists |

Extraction is not free. Move the smallest thing that both callers need, at the lowest layer that can express it without dragging a dependency upward.

### Mechanics

- **Prefer concrete types, then generics, then traits.** `&str` and `&[T]` before `impl AsRef<str>`; a generic parameter before `dyn`; a trait object only at a boundary that must be swapped at runtime, such as a clock, a transport or a provider. One implementation means no trait.
- **Newtypes make the share safe.** `fn move_item(item: ItemId, into: GroupId)` beats two `Uuid` parameters that can be swapped. Same bytes, different meaning, different type.
- **Share the contract once.** The wire type is defined in the contract crate and used by both ends. Never hand-copy a DTO, recompute an accepted count on the client, or hard-code a server limit in the view.
- **Feature flags are for targets, not for rules.** A business rule behind a cargo feature is two products with one name. `cfg` is for platform APIs and wasm/native, and every claimed target must compile with a real fallback.
- **Shared controls stay domain-free.** Display values, state and events in the public API; no database, routing, services or fixtures; a documented keyboard and focus contract; a synthetic example; an interaction test; and it builds native and wasm.
- **Share by meaning, not by shape.** A board name and a group name are both `String`, but only share a rule if the rule is identical. A page limit and an upload limit are both `usize` and must not share a constant.

### Extraction protocol

1. Trace both callers to the rule they actually need, and confirm it is the same rule.
2. Move it to the chosen owner, with its tests.
3. Switch the equivalent callers in the same change; do not leave a parallel path behind.
4. Verify at the boundary where drift would show: the store for persistence, the host for authorization, a running control for focus and input, both clients for the wire.
5. Preserve stored and wire formats, or migrate them explicitly.
6. Update the owner map and delete the duplicate.

When you deliberately keep two implementations, state the concrete difference: lifetime, validation, storage, change cadence, or audience. "Similar code" is not a difference.

### What not to share

- A 50-row read page and a 50-file upload queue: same number, different risks.
- A view draft and a persisted value: different owners, different failure handling.
- Two entities that happen to have a title now, but are renamed, archived or permissioned differently.
- A helper whose only shared trait is "it was convenient at the time".

## 4. Rust Best Practice

### Types and ownership

- Make invalid states unrepresentable: newtypes for identity, enums instead of sets of booleans, `Option` and `Result` instead of sentinels, constructors that validate (`ItemName::parse(raw)?`).
- Take the least you need: `&str` over `&String`, `&[T]` over `&Vec<T>`, `impl AsRef<Path>` over `PathBuf`. Return owned data only when the caller keeps it.
- Clone deliberately. `Arc<T>` at a UI or hand-off boundary, not sprayed about to quiet the borrow checker.
- Derive in a stable order: `Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize`. `Debug` everywhere except where it would print a secret.
- `#[non_exhaustive]` on public error and catalogue enums that will grow. `#[must_use]` on anything whose loss is a silent bug.
- No `unsafe` in workspace crates. If a dependency needs it, isolate it in the crate that needs it and say so.
- No panics on runtime paths: no `unwrap`, `expect`, indexing or `panic!` on user input, database rows or file content. `expect` is for programmer invariants with an explanatory message. Use `let ... else`, checked arithmetic and explicit bounds.

### Errors

- One error enum per boundary (`StoreError`, `HostError`, `SessionError`), derived from `thiserror`, with data-carrying variants, not pre-formatted prose.
- Distinguish validation, missing record, conflict and unexpected fault in the type. Map to HTTP, exit code or UI message only at the edge.
- Convert with `From` at the boundary. No `Box<dyn Error>` in a public library API.
- Expected outcomes are values, not errors: "no boards" is `Ok(Vec::new())` plus a reason; "database unreachable" is `Err`. Cancellation is its own variant, not a failure.
- Log an unexpected failure once, at its owning boundary, with the operation and identifiers, and never twice on the way up.

### Concurrency and blocking

- Database, file and network work never runs on the UI thread. In this workspace the clients are synchronous (`postgres`, `ureq`, `tiny_http`), so they run on a worker; the GPUI event loop only paints and dispatches.
- Prefer message passing or a bounded channel to shared mutable state. `Arc<Mutex<_>>` needs a stated reason.
- Never hold a lock across a transaction commit, an I/O call, an await point or a UI callback.
- Bound queues. Dropping work with a counter beats unbounded growth.
- Claim and release work explicitly; a restarted worker must not lose a claimed job or repeat a protected effect.

### Database

- One transaction per accepted command. Report success after commit, never before.
- Bounded, ordered reads: an explicit `LIMIT`, and an `ORDER BY` with a total order (position, then identity, then creation).
- Constraints in the schema (foreign key, unique, check) as well as in code. Migrations are append-only and owned by the crate.
- Concurrency by revision column or row lock, chosen and documented; serialising inside one process is not durability.
- Batch lookups; check for a query per row before reaching for a cache.

### Cargo and dependencies

- Workspace inheritance for package fields, dependencies and lints. One version of everything in the graph.
- `--locked` everywhere; the lockfile is part of the change when dependencies move.
- Add a dependency for a real need, read its licence and its `unsafe` posture, and prefer one already in the graph. Ask the owner before a heavyweight addition or any licence selection.
- Exact `rev` for git sources; record why the fork or patch exists.
- One active build at a time; concurrent builds thrash the target directory and the machine.

### Lints, docs and API surface

- `cargo fmt` decides formatting in every workspace. Do not hand-format or restructure logic to please it.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` is a gate, not advice.
- `#![warn(missing_docs)]` on library crates: `//!` says who owns the module, `///` says the contract (inputs, accepted result, failure, transaction, callers).
- Keep the public surface small and documented. A `pub` item without a caller is a promise nobody asked for.
- Lint config lives in `clippy.toml` and `[workspace.lints]`, with test exemptions (`allow-unwrap-in-tests`) so tests stay readable without weakening production rules.

### Logging

- Use the `log` facade with structured fields: operation, entity identifiers, outcome. No passwords, tokens, session values or user payloads.
- One line for an accepted command, one for an unexpected failure. Keep expected rejections out of the error log.
- A failure the user must act on is a UI state with a retry, not a log line. Operator recovery must be discoverable from the record, not from a comment.

### Tests

- A test earns its place by guarding behaviour a plausible change could break, and no other test catches. Do not test wiring, constants or a library's own behaviour.
- Test at the boundary where a mistake shows: the store for persistence, the host for authorization, a running control for input, focus and recovery. Compilation does not verify keyboard input.
- Assert identity, order and outcome ("the second row kept its ID and changed only its title"). Use exact values only where the contract is the observation, such as a returned UUID, revision or HTTP status.
- A bug fix adds a case that fails without it, unless the fix is a few lines a reader can verify.
- Deterministic by construction: fixed clock, fixed IDs where possible, no sleeps for synchronisation, no dependency on wall time or test order.
- Integration tests use a disposable real database; do not mock the store where the transaction is the thing under test.
- Never delete, weaken, `--ignore` or filter a test to make a gate pass. Report a suite as passing only when it ran.

## Implement One Vertical Feature

1. **Place it.** Decide the crate, layer and module before writing code. Does it reuse an existing owner, extend one, or need a new crate (recorded architecture decision)? If the answer is a new module in a dumping ground, stop and name the concept instead.
2. **State the contract briefly.** The user action, access scope, authoritative records, success condition and most important failure. No design document for a routine field edit.
3. **Choose the owner.** One named Rust operation for the business decision, reused by every action that means the same thing. Identify shared policy, and extract it in this change when a real second caller exists.
4. **Model the data deliberately.** Stable identifiers as newtypes, explicit relationships, constraints and migrations. Choose variable-value representations from real query and validation needs, not a universal `serde_json::Value` bag.
5. **Implement the commit.** Validate server-side, enforce the real scope and conflict policy, persist, then return identity or revision. Everything that must succeed together commits together.
6. **Handle consequences.** Decide whether a follow-up is best effort or required. Required work is written durably in the same transaction; two independent writes are not atomic.
7. **Complete the UI.** Loading, empty, validation, saving, conflict and recoverable failure as applicable. The draft reconciles with the accepted result or is rolled back visibly. Database work stays off the UI thread.
8. **Verify and leave a trail.** Focused behaviour tests, the layer table still true in one direction, and a handoff naming the owner, the checks that ran and what remains unverified.

## Worked Decision: Rename An Item

An implementation guide for the shape a feature takes, not an instruction to scaffold it:

1. **Place it.** No new crate. The rule belongs in the store's item module; the host route is a handler, not a second rule; the draft belongs to the view. The layer table does not move.
2. **Send the contract.** The view submits the item ID, the intended spelling and the revision it read, through the type the contract crate already defines. No parallel DTO.
3. **Authorize and delegate.** The thin host handler resolves the session and checks current board access, then calls the named operation. It formats an outcome; it decides nothing.
4. **Commit once.** The operation checks ownership, the entry revision and the accepted title, trims and validates, writes the row and advances the revision in one transaction, and returns the committed row. It reports success only after commit, and a repeated identical command is acknowledged rather than applied twice.
5. **Reconcile the view.** Only the accepted row replaces the displayed value. A rejection keeps the draft with a retry and a dismiss; a stale revision is a conflict the user resolves, never a silent overwrite.
6. **Treat live signals as freshness.** Publish after commit so other clients can refetch. A missed signal costs a refresh, not the edit.
7. **Verify where it shows.** Two rows with the same title, rename one, reconnect, check both IDs and the order. Reject a blank name and keep the draft. Lose write access mid-edit and see a denial, not a partial save.

The architecture question is whether the rule has one owner and the dependency still points down. The workflow question is whether a repeated action and a partial failure leave the user and the operator in a recoverable state. One implementation answers both.

## Resolve Tensions Explicitly

- **Shared vs separate:** share a rule once its meaning is identical at both call sites; keep two implementations when lifetime, validation, storage or change cadence differ, and state which one.
- **Simple vs recoverable:** keep the direct synchronous commit for the user action; add durable follow-up only when a real consequence needs it. Do not trade correctness for fewer modules.
- **Fast vs correct:** bounded, well-shaped queries first; caching and concurrency arrive with ownership, invalidation and failure semantics attached.
- **Clean tree vs delivered feature:** a new module in the right crate beats a new crate in the wrong layer. If the right home does not exist yet, raise it as a recorded architecture decision rather than inventing `common` locally.
- **Responsive vs ordered:** queue I/O and keep sequencing at the owning operation; do not block the UI, and do not scatter detached work.
- **Compatibility vs clean design:** preserve real consumers and stored data with a bounded migration; do not preserve complexity that has no current consumer.
- **Modern vs familiar:** established crates behind ordinary domain names. New libraries are welcome; novel internal frameworks are not.

## Definition Of Done

A substantive feature is ready when the normal workflow works end to end, the rule has one owner, the workspace graph still points only downward, no shared rule has been copied, durable state and the important failure transitions are tested, queries are bounded, and another engineer can find the rule and diagnose a failure from the record.

Run the gates this workspace asks for, and report what actually ran:

```text
cargo fmt --all -- --check
cargo fmt --manifest-path android/Cargo.toml -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace --bins
```

Add the layering check where it exists, the native and wasm builds for shared controls, and the focused acceptance test for the workflow you touched. One build at a time. Never report a suite as passing that did not run.

Scale verification to risk. Do not claim coverage, security approval or device acceptance that was not exercised. One coherent implementation with concise evidence beats two fictional reviews.

For consequential choices, record the decision, the reason, the affected owner and the operational implication in the existing project documentation, when the document is one a feature change may not edit. The handoff names the entry point, the rule owner, the persistence and migration, the shared code reused or extracted, and the checks that passed.

