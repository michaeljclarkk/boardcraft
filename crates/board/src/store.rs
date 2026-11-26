//! Blocking PostgreSQL operations. Call these on a worker, not the editor thread.
use postgres::{Client, Error};

use crate::{Board, BoardId, Item, ItemId, RenameError, interactive_title};

/// Apply each schema change and record its version in the same transaction.
pub fn migrate(client: &mut Client) -> Result<(), Error> {
    let mut tx = client.transaction()?;
    tx.batch_execute(
        "SELECT pg_advisory_xact_lock(hashtext('rustday.board.migrations'));
         CREATE TABLE IF NOT EXISTS rustday_migrations (version integer PRIMARY KEY)",
    )?;
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version = 1",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0001_boards.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (1)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version = 2",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0002_item_revision.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (2)", &[])?;
    }
    tx.commit()
}

/// Create a board snapshot without rewriting IDs, titles or row order.
pub fn create_board(client: &mut Client, board: &Board) -> Result<(), Error> {
    let mut tx = client.transaction()?;
    tx.execute("INSERT INTO boards (id) VALUES ($1)", &[&board.id.0])?;
    let ids: Vec<_> = board.items.iter().map(|item| item.id.0).collect();
    let titles: Vec<_> = board.items.iter().map(|item| item.title.as_str()).collect();
    tx.execute(
        "INSERT INTO board_items (board_id, id, title, position)
         SELECT $1, id, title, n - 1 FROM unnest($2::uuid[], $3::text[])
         WITH ORDINALITY AS items(id, title, n)",
        &[&board.id.0, &ids, &titles],
    )?;
    tx.commit()
}

pub fn load_board(client: &mut Client, id: BoardId) -> Result<Option<Board>, Error> {
    Ok(load_items(client, id)?.map(|rows| Board {
        id,
        items: rows
            .into_iter()
            .map(|row| Item {
                id: row.id,
                title: row.title,
            })
            .collect(),
    }))
}

pub struct StoredItem {
    pub id: ItemId,
    pub title: String,
    pub revision: i64,
}

pub fn load_items(client: &mut Client, id: BoardId) -> Result<Option<Vec<StoredItem>>, Error> {
    // One snapshot; the outer join distinguishes an empty board from an absent one.
    let rows = client.query(
        "SELECT i.id, i.title, i.revision FROM boards b LEFT JOIN board_items i ON i.board_id = b.id
         WHERE b.id = $1 ORDER BY i.position",
        &[&id.0],
    )?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        if let Some(uuid) = row.try_get(0)? {
            items.push(StoredItem {
                id: ItemId(uuid),
                title: row.try_get(1)?,
                revision: row.try_get(2)?,
            });
        }
    }
    Ok(Some(items))
}

#[derive(Debug)]
pub enum SaveError {
    Rejected(RenameError),
    Conflict { current: String, revision: i64 },
    Database(Error),
}

/// Re-read just the name targeted by a user-requested retry.
pub fn current_item(
    client: &mut Client,
    board: BoardId,
    item: ItemId,
) -> Result<StoredItem, SaveError> {
    client
        .query_opt(
            "SELECT title, revision FROM board_items WHERE board_id=$1 AND id=$2",
            &[&board.0, &item.0],
        )
        .and_then(|row| {
            row.map(|r| {
                Ok(StoredItem {
                    id: item,
                    title: r.try_get(0)?,
                    revision: r.try_get(1)?,
                })
            })
            .transpose()
        })
        .map_err(|error| {
            log::error!(
                "current_title board={} item={} sqlstate={:?}",
                board.0,
                item.0,
                error.code()
            );
            SaveError::Database(error)
        })?
        .ok_or(SaveError::Rejected(RenameError::MissingItem(item)))
}

pub fn rename_item(
    client: &mut Client,
    board: BoardId,
    item: ItemId,
    draft: &str,
) -> Result<String, SaveError> {
    save_title(client, board, item, Condition::Any, draft).map(|row| row.title)
}

/// An inline editor must not silently overwrite a title changed since it loaded.
pub fn rename_item_if_current(
    client: &mut Client,
    board: BoardId,
    item: ItemId,
    expected: &str,
    draft: &str,
) -> Result<String, SaveError> {
    save_title(client, board, item, Condition::Title(expected), draft).map(|row| row.title)
}

/// An older positive revision is valid for a name-only edit; a future one is not.
pub fn rename_item_at_revision(
    client: &mut Client,
    board: BoardId,
    item: ItemId,
    revision: i64,
    draft: &str,
) -> Result<StoredItem, SaveError> {
    save_title(
        client,
        board,
        item,
        Condition::KnownRevision(revision),
        draft,
    )
}

/// Finish a name-only edit from its entry revision, guarded by the last title
/// actually loaded in this Windows view. Check both on the same locked row.
pub fn finish_name_edit(
    client: &mut Client,
    board: BoardId,
    item: ItemId,
    revision: i64,
    observed: &str,
    raw: &str,
) -> Result<StoredItem, SaveError> {
    save_title(
        client,
        board,
        item,
        Condition::ObservedName(revision, observed),
        raw,
    )
}

enum Condition<'a> {
    Any,
    Title(&'a str),
    KnownRevision(i64),
    ObservedName(i64, &'a str),
}

fn save_title(
    client: &mut Client,
    board: BoardId,
    item: ItemId,
    condition: Condition<'_>,
    draft: &str,
) -> Result<StoredItem, SaveError> {
    let title = interactive_title(draft).map_err(SaveError::Rejected)?;
    let fault = |error: Error| {
        log::error!(
            "rename_item board={} item={} sqlstate={:?}",
            board.0,
            item.0,
            error.code()
        );
        SaveError::Database(error)
    };
    let mut tx = client.transaction().map_err(fault)?;
    let row = tx
        .query_opt(
            "SELECT title, revision FROM board_items WHERE board_id = $1 AND id = $2 FOR UPDATE",
            &[&board.0, &item.0],
        )
        .map_err(fault)?
        .ok_or(SaveError::Rejected(RenameError::MissingItem(item)))?;
    let current: String = row.try_get(0).map_err(fault)?;
    let revision: i64 = row.try_get(1).map_err(fault)?;
    let allowed = match condition {
        Condition::Any => true,
        Condition::Title(expected) => expected == current,
        Condition::KnownRevision(r) => (1..=revision).contains(&r),
        Condition::ObservedName(r, observed) => observed == current && (1..=revision).contains(&r),
    };
    if !allowed {
        return Err(SaveError::Conflict { current, revision });
    }
    let mut saved = StoredItem {
        id: item,
        title: current,
        revision,
    };
    if title != saved.title {
        let updated = tx.query_one(
            "UPDATE board_items SET title = $3, revision = revision + 1 WHERE board_id = $1 AND id = $2 RETURNING title, revision",
            &[&board.0, &item.0, &title],
        ).map_err(fault)?;
        saved.title = updated.try_get(0).map_err(fault)?;
        saved.revision = updated.try_get(1).map_err(fault)?;
    }
    tx.commit().map_err(fault)?;
    Ok(saved)
}
