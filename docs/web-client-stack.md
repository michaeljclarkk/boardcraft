# Browser client stack: GPUI on WebAssembly

**Status:** decided 2026-11-27 by the owner. Implementation is queued in packets 53–57. The JavaScript preview remains the only working browser client until packet 54 lands.
**Supersedes:** the JavaScript preview as the target browser client. It does not change the host contract, the store rules or the native clients.

## Decision

1. **One UI toolkit.** Desktop, Android and the browser render with GPUI Kit. The target release is `gpui-kit = "=0.11.0"` in the workspace, `crates/boardcraft-ui` and `android/`.
2. **The browser client is Rust WebAssembly.** It lives in `crates/web` (package `boardcraft-web`) and targets `wasm32-unknown-unknown`. The existing loopback host serves it on the same origin as the preview.
3. **`crates/boardcraft-ui` is the only shared control crate.** It MUST compile for `wasm32-unknown-unknown` and MUST NOT depend on the desktop binary, domain entities, database, routing, services or native-only APIs.
4. **The web shell is separate from `crates/desktop`.** It owns one canvas window, fonts, embedded assets and HTTP calls. It MUST NOT depend on the `windows` crate, native windowing or the blocking `postgres` client.
5. **Browser UI source is Rust.** Only the minimal Trunk bootstrap page and generated `wasm-bindgen` glue may exist as HTML/JavaScript, and neither contains product logic. No hand-written browser application JavaScript is added after this decision. Packet 57 removes the JavaScript preview completely; no parallel client, fallback route or feature flag is retained.
6. **Browser verification targets the accessibility tree.** Checks assert role, name, state and action plus accepted application state. DOM selectors from the JavaScript preview are retired with it.
7. **Tooling is out of scope.** The Node runner and `scripts/*.mjs` delivery and test tooling are not part of this decision. This is about the browser client, not the repository's tooling language.

## Why

- The host and store already own the rules and the session; only the view layer is duplicated.
- One toolkit removes a second implementation of focus, drag, drafts and recovery.
- The current preview is about 1,300 lines of hand-written UI plus about 1,700 lines of DOM-coupled tests. Replacing it early is cheaper than re-porting every later workflow.
- GPUI Kit advertises WebAssembly support on `wasm32-unknown-unknown`, and its dependency graph already includes a web platform crate at the pinned release.

## What is unchanged

- Business rules in `crates/board`; access decisions in `crates/mobile-edit-host`; PostgreSQL durability.
- The loopback origin, the host-owned bearer, the HttpOnly session cookie and the exact-origin binding.
- Native clients, their contracts and their existing acceptance evidence.
- Every workflow acceptance target. Only the browser implementation and its evidence mechanism change.

## Constraints and risks

| Risk | Impact | Verification / mitigation |
|---|---|---|
| `gpui-mobile` compatibility with Kit 0.11.0 | Android is pinned to Kit 0.8.1 and a git revision of the mobile host; the mobile guide documents an older core ([runbook](development.md#android-native-host)). A version bump may not compile. | Verify before packet 53. If no compatible mobile host exists, stop and re-decide; do not fork silently or ship two toolkits. |
| Web backend maturity | One canvas window, no system fonts, no filesystem, workers need cross-origin isolation. | Prove the shell and one full workflow in packet 53/54 before deleting anything. |
| Payload size | Third-party reports put GPUI wasm builds at roughly 10–15 MB. | Measure this build and record first-load time; do not claim parity without the number. |
| Test mechanism | A canvas has no queryable DOM controls. | Require an accessibility tree with roles and names for every control before packet 57 deletes the DOM harness. |
| Native-only components | Some Kit layers are documented native-only. | Reimplement the missing interaction in `boardcraft-ui` with a synthetic example; do not bypass the shared control with a web-only widget. |
| Accessibility | A canvas app can look complete while being unusable with assistive technology. | Assert role, name, state, keyboard path and focus in packet 56; treat missing semantics as a failure. |

## Migration

| Packet | Scope | Exit |
|---|---|---|
| [53-web-shell](../packets/53-web-shell.md) | `crates/web` wasm build, host route, cross-origin isolation, fonts, assets, one window, session binding | The wasm page loads on loopback and resolves an existing session |
| [54-web-auth-board](../packets/54-web-auth-board.md) | Sign-in, board list, selected board read, rename, rejected-save recovery | Packets 26, 27, 29 and 30 re-verified on wasm |
| [55-web-groups](../packets/55-web-groups.md) | Groups, item create, move, reorder, selection, archive and Trash | Packets 31–43, 51 and 52 re-verified on wasm |
| [56-web-test-harness](../packets/56-web-test-harness.md) | Accessibility-tree driver replacing the DOM driver | Every browser row has a named check against the wasm client |
| [57-delete-js-preview](../packets/57-delete-js-preview.md) | Remove the preview assets, router, mode, routes, feature, example and harness | The preview is gone, not disabled; one browser client remains |

Rules during migration:

- The JavaScript preview is closed to new workflows. It receives only critical fixes and the existing queue.
- Do not mark a new browser row verified against the JavaScript preview after 2026-11-27. Re-verify the existing rows under the wasm client.
- Do not start packet 57 until the auth and group packets pass and the accessibility harness covers the affected rows.
- Packet 57 removes the preview completely. No fallback route, redirect, feature flag or second client remains afterwards. A blocked packet means the migration is unfinished, not that two clients are acceptable.
- Keep the host session table, the HttpOnly cookie and exact-origin binding, the access policy and the board HTTP operations. They are host code, not preview code. Migrate host tests that use `Host::start_preview` or `/preview/api` to the wasm route instead of deleting their coverage. Do not port the DOM assertions; the wasm harness is new.

## Removal inventory

Packet 57 must remove all of the following, not only the page assets:

| Area | Current location |
|---|---|
| Preview assets | `crates/mobile-edit-host/src/preview/` (JavaScript, `index.html`, `page.css`) |
| Asset router | `crates/mobile-edit-host/src/preview.rs` |
| Host mode and entry | `Mode::Preview`, `Host::start_preview`, the `/preview` and `/preview/api` branches in `crates/mobile-edit-host/src/lib.rs` |
| Preview naming | Rename the `Boardcraft-Preview` header and binding to the web client's name; remove any `preview` identifier that describes the deleted client |
| DOM test harness | `crates/mobile-edit-host/tests/browser_preview.rs` and `crates/mobile-edit-host/tests/browser/*.mjs` — deleted, not ported |
| Build wiring | The `browser-smoke` feature, `browser_preview` test target and `browser_board_preview` example in `crates/mobile-edit-host/Cargo.toml` |
| Documentation | Runbook commands, architecture rows and register links that point at the preview |

The `browser_session` table, cookie and binding mechanics, access policy, board operations and their fixture checks stay and are migrated to the wasm client's route.

The DOM assertions are not migrated. Copying selector-based tests would encode the deleted UI; the wasm client gets a new accessibility-tree harness in packet 56. Host-contract tests that happen to call `Host::start_preview` or `/preview/api` are not browser UI tests — migrate them, because they cover access, revisions, replay and rollback that the wasm client also depends on.

The `browser_preview` gate in `scripts/run.mjs` and its cases in `scripts/run.test.mjs` are coordinator-owned. Retire or rewire them at the same milestone; a worker packet MUST NOT edit the runner.

## Consequences for contributors

- Browser changes touch `crates/web` and `crates/boardcraft-ui`, not `crates/mobile-edit-host/src/preview`.
- A shared control change is checked on desktop and wasm; the handoff names the target that was exercised.
- Do not add a second toolkit, a React/JavaScript view layer, or a web-only control to work around a shared one.
- A missing Kit interaction is added to `crates/boardcraft-ui` with docs, an example, an interaction test and accessibility semantics. Do not embed HTML/CSS/JS/React/TSX, a webview, iframe or DOM overlay.
- Keep custom controls domain-free and documented so they can be shared or proposed upstream later. An upstream PR is not part of a packet ([shared GPUI components](gpui-components.md)).
