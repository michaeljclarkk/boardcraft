use super::{BoardId, GROUPED_ROW_LIMIT, ItemId};
use postgres::{Error, Transaction, types::ToSql};
use uuid::Uuid;

pub struct Reorder<'a> {
    pub operation: Uuid,
    pub group: &'a str,
    pub anchor: Uuid,
    pub after: bool,
    pub board_revision: i64,
    pub item_revision: i64,
}

#[derive(Debug)]
pub enum ReorderError {
    Invalid,
    Missing,
    Conflict,
    TooLarge,
    Database(Error),
}
impl From<Error> for ReorderError {
    fn from(e: Error) -> Self {
        Self::Database(e)
    }
}

/// Caller owns current write authorization and the transaction. The board lock
/// serializes ordering commands, including two targeting different groups.
pub fn reorder_group_item(
    tx: &mut Transaction<'_>,
    actor: Uuid,
    board: BoardId,
    item: ItemId,
    r: &Reorder<'_>,
) -> Result<(), ReorderError> {
    use ReorderError::*;
    let revision = lock_board(tx, board)?;
    let args: &[&(dyn ToSql + Sync)] = &[
        &r.operation,
        &actor,
        &board.0,
        &item.0,
        &r.group,
        &r.anchor,
        &r.after,
        &r.board_revision,
        &r.item_revision,
    ];
    let receipt = tx.query_opt(
        "SELECT (actor_id,board_id,item_id,group_id,anchor_id,after_anchor,board_revision,item_revision)
                IS NOT DISTINCT FROM ($2,$3,$4,$5,$6,$7,$8,$9)
         FROM group_reorder_receipts WHERE operation_id=$1", args)?;
    if let Some(receipt) = receipt {
        return if receipt.get(0) {
            Ok(())
        } else {
            Err(Conflict)
        };
    }
    if r.operation.is_nil() || r.anchor == item.0 || r.anchor.is_nil() {
        return Err(Invalid);
    }
    if revision != r.board_revision || r.item_revision < 1 {
        return Err(Conflict);
    }
    let rows = lock_group_order(tx, board, r.group)?;
    let mut order: Vec<Uuid> = rows.iter().map(|r| r.get(0)).collect();
    let source = order.iter().position(|id| *id == item.0).ok_or(Missing)?;
    if rows[source].get::<_, i64>(1) != r.item_revision {
        return Err(Conflict);
    }
    order.remove(source);
    let anchor = order
        .iter()
        .position(|id| *id == r.anchor)
        .ok_or(Conflict)?;
    order.insert(anchor + usize::from(r.after), item.0);

    // Another board may have claimed the same operation UUID concurrently.
    if tx.execute("INSERT INTO group_reorder_receipts (operation_id,actor_id,board_id,item_id,group_id,anchor_id,after_anchor,board_revision,item_revision) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT DO NOTHING", args)? != 1 { return Err(Conflict); }
    renumber(tx, &order)?;
    tx.execute("UPDATE board_items SET revision=revision+1,group_revision=revision+1,position_revision=revision+1 WHERE id=$1", &[&item.0])?;
    tx.execute(
        "UPDATE boards SET revision=revision+1 WHERE id=$1",
        &[&board.0],
    )?;
    Ok(())
}

pub(super) fn lock_board(tx: &mut Transaction<'_>, board: BoardId) -> Result<i64, ReorderError> {
    Ok(tx
        .query_opt(
            "SELECT revision FROM boards WHERE id=$1 FOR NO KEY UPDATE",
            &[&board.0],
        )?
        .ok_or(ReorderError::Missing)?
        .get(0))
}

pub(super) fn lock_group_order(
    tx: &mut Transaction<'_>,
    board: BoardId,
    group: &str,
) -> Result<Vec<postgres::Row>, ReorderError> {
    tx.query_opt(
        "SELECT id FROM board_groups WHERE board_id=$1 AND id=$2 AND active FOR SHARE",
        &[&board.0, &group],
    )?
    .ok_or(ReorderError::Invalid)?;
    let rows = tx.query(
        "SELECT id,revision FROM board_items WHERE board_id=$1 AND group_id=$2
         ORDER BY position,created_at NULLS FIRST,id LIMIT $3 FOR UPDATE",
        &[&board.0, &group, &(GROUPED_ROW_LIMIT + 1)],
    )?;
    if rows.len() > GROUPED_ROW_LIMIT as usize {
        return Err(ReorderError::TooLarge);
    }
    Ok(rows)
}

pub(super) fn renumber(tx: &mut Transaction<'_>, order: &[Uuid]) -> Result<(), Error> {
    tx.execute("UPDATE board_items i SET position=o.n-1 FROM unnest($1::uuid[]) WITH ORDINALITY AS o(id,n) WHERE i.id=o.id AND i.position<>o.n-1", &[&order])?;
    Ok(())
}
