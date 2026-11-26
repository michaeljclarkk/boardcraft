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
    // One snapshot; the outer join distinguishes an empty board from an absent one.
    let rows = client.query(
        "SELECT i.id, i.title FROM boards b LEFT JOIN board_items i ON i.board_id = b.id
         WHERE b.id = $1 ORDER BY i.position",
        &[&id.0],
    )?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut board = Board::new(id);
    for row in rows {
        if let Some(uuid) = row.try_get(0)? {
            board.items.push(Item {
                id: ItemId(uuid),
                title: row.try_get(1)?,
            });
        }
    }
    Ok(Some(board))
}

#[derive(Debug)]
pub enum SaveError {
    Rejected(RenameError),
    Database(Error),
}

pub fn rename_item(
    client: &mut Client,
    board: BoardId,
    item: ItemId,
    draft: &str,
) -> Result<String, SaveError> {
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
    let saved = tx
        .query_opt(
            "UPDATE board_items SET title = $3 WHERE board_id = $1 AND id = $2 RETURNING title",
            &[&board.0, &item.0, &title],
        )
        .map_err(fault)?;
    let row = saved.ok_or(SaveError::Rejected(RenameError::MissingItem(item)))?;
    let accepted = row.try_get(0).map_err(fault)?;
    tx.commit().map_err(fault)?;
    Ok(accepted)
}
