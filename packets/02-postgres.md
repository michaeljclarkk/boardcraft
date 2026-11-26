# 02-postgres

Add migrations and persist the board operation in PostgreSQL. Test on a dedicated, disposable boardcraft database supplied as `RUSTDAY_TEST_DATABASE_URL`. No fallback to other URLs or databases. In `postgres_board`, save an edit, reconnect, reload the board and verify identity, ordering and values. Include a failed-write case proving no partial edit is visible. Never skip the test on a missing database.

Use nonempty UUIDs for durable board and item identities so the store can round-trip existing IDs. Keep the rename scoped to its board. For interactive edits, store the trimmed title; reject blank titles and titles over 255 UTF-16 code units after trimming. Exercise these rules with synthetic inputs and distinguish rejected edits from database faults. Historical imports have a separate spelling contract.
