# Deployment shape

Documented target for shared and delivery use. Not implemented yet: do not add a Dockerfile, Compose file or CI packaging until the owner asks.

## Two containers

| Service | Role | Port |
|---|---|---|
| `boardcraft` | The server, API and Rust/WASM assets | **5078**, published |
| `db` | PostgreSQL 17 today; pgrust only after the [deferred move](deferred.md) is triggered | 5432, internal only |

Clients — desktop, browser and Android — connect to `http://<host>:5078`. They never connect to the database directly.

## Compose contract

- One `compose.yaml` at the repository root, two services, named volumes.
- `boardcraft-data` for application data; `db-data` for the database.
- The database publishes no host port. Inspect it with `docker compose exec db psql`.
- The app waits for a database healthcheck, then applies migrations once and fails fast on error.
- Values come from the environment or `.env`; no credentials or private paths in images or layers.
- Restart policy `unless-stopped`; stop grace period long enough to finish a commit.

## Network and access

- Development stays source-based and loopback-only.
- Shared use runs the app on `5078`, reachable on the private interface or tailnet; TLS terminates at the existing reverse proxy.
- No public ingress before authentication, session handling and access rules are in place.
- Keep the database private. Only the app container joins its network.

## Current status

Local development runs from source with `tools/` helpers and a disposable PostgreSQL cluster. The container shape above is the target for a shared server; it is documentation, not work to start now.
