use super::{
    BoardId, GROUPED_ROW_LIMIT, ReorderError,
    reorder::{lock_board, lock_group_order, renumber},
};
use postgres::{Transaction, types::ToSql};
use std::collections::HashSet;
use uuid::Uuid;

pub struct SelectionMove<'a> {
    pub operation: Uuid,
    pub group: &'a str,
    pub board_revision: i64,
    pub items: &'a [(Uuid, i64)],
}

/// Append the selected block in command order. The caller holds current write access.
pub fn move_selected_items(
    tx: &mut Transaction<'_>,
    actor: Uuid,
    board: BoardId,
    cmd: &SelectionMove<'_>,
) -> Result<Vec<Uuid>, ReorderError> {
    use ReorderError::*;
    if cmd.operation.is_nil() || !(1..=10_000).contains(&cmd.items.len()) {
        return Err(Invalid);
    }
    let (ids, revisions): (Vec<_>, Vec<_>) = cmd.items.iter().copied().unzip();
    let selected: HashSet<_> = ids.iter().copied().collect();
    if selected.len() != ids.len() || selected.contains(&Uuid::nil()) {
        return Err(Invalid);
    }
    let revision = lock_board(tx, board)?;
    let args: &[&(dyn ToSql + Sync)] = &[
        &cmd.operation,
        &actor,
        &board.0,
        &cmd.group,
        &cmd.board_revision,
        &ids,
        &revisions,
    ];
    let receipt = tx.query_opt(
        "SELECT (actor_id,board_id,group_id,board_revision,selected_ids,selected_revisions)
                IS NOT DISTINCT FROM ($2,$3,$4,$5,$6,$7), affected_ids
         FROM group_reorder_receipts WHERE operation_id=$1",
        args,
    )?;
    if let Some(row) = receipt {
        return if row.get(0) {
            Ok(row.get(1))
        } else {
            Err(Conflict)
        };
    }
    if revision != cmd.board_revision {
        return Err(Conflict);
    }
    check_grid_budget(tx, board)?;
    let siblings = lock_group_order(tx, board, cmd.group)?;
    let roots = tx.query(
        "SELECT i.id,i.revision FROM unnest($2::uuid[]) WITH ORDINALITY AS r(id,n)
        JOIN board_items i ON i.board_id=$1 AND i.id=r.id AND i.state=0
        JOIN board_groups g ON g.board_id=i.board_id AND g.id=i.group_id AND g.active
        ORDER BY r.n FOR UPDATE OF i FOR SHARE OF g",
        &[&board.0, &ids],
    )?;
    if roots.len() != ids.len() {
        return Err(Missing);
    }
    if roots
        .iter()
        .zip(&revisions)
        .any(|(r, rev)| r.get::<_, i64>(1) != *rev)
    {
        return Err(Conflict);
    }
    let retained: Vec<Uuid> = siblings
        .iter()
        .map(|r| r.get(0))
        .filter(|id| !selected.contains(id))
        .collect();
    renumber(tx, &retained)?;
    let affected: Vec<Uuid> = tx
        .query(
            "UPDATE board_items i SET group_id=$2, position=$3+o.n-1,
             revision=revision+1,group_revision=revision+1,position_revision=revision+1
         FROM unnest($1::uuid[]) WITH ORDINALITY o(id,n) WHERE i.id=o.id RETURNING i.id",
            &[&ids, &cmd.group, &(retained.len() as i64)],
        )?
        .iter()
        .map(|r| r.get(0))
        .collect();
    // Empty source groups also consume projection rows. Reject before accepting a receipt.
    check_grid_budget(tx, board)?;
    let mut receipt_args = args.to_vec();
    receipt_args.push(&affected);
    let inserted = tx.execute(
        "INSERT INTO group_reorder_receipts (operation_id,actor_id,board_id,group_id,board_revision,
         selected_ids,selected_revisions,affected_ids) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
         ON CONFLICT DO NOTHING", &receipt_args)?;
    if inserted != 1 {
        return Err(Conflict);
    }
    tx.execute(
        "UPDATE boards SET revision=revision+1 WHERE id=$1",
        &[&board.0],
    )?;
    Ok(affected)
}

pub(super) fn check_grid_budget(
    tx: &mut Transaction<'_>,
    board: BoardId,
) -> Result<(), ReorderError> {
    let overflow = tx.query_opt(
        "SELECT 1 FROM board_groups g LEFT JOIN board_items i
        ON (i.board_id,i.group_id)=(g.board_id,g.id) AND i.state=0 WHERE g.board_id=$1 AND g.active
        OFFSET $2 LIMIT 1",
        &[&board.0, &GROUPED_ROW_LIMIT],
    )?;
    if overflow.is_some() {
        Err(ReorderError::TooLarge)
    } else {
        Ok(())
    }
}
