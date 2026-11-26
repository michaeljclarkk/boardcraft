use super::{
    BoardId, GROUPED_ROW_LIMIT, ReorderError, reorder::lock_board,
    selection_move::check_grid_budget,
};
use postgres::Transaction;
use std::collections::HashSet;

/// Caller holds selected Owner authorization through commit. An explicit order
/// advances every active group's revision, even for a same-position bystander.
pub fn reorder_groups(
    tx: &mut Transaction<'_>,
    board: BoardId,
    baseline: i64,
    ids: &[String],
) -> Result<(), ReorderError> {
    let current = lock_board(tx, board)?;
    if baseline < 1 {
        return Err(ReorderError::Invalid);
    }
    if baseline != current {
        return Err(ReorderError::Conflict);
    }
    if ids.len() > GROUPED_ROW_LIMIT as usize {
        return Err(ReorderError::TooLarge);
    }
    check_grid_budget(tx, board)?;
    let rows = tx.query("SELECT id FROM board_groups WHERE board_id=$1 AND active AND NOT deleted ORDER BY id LIMIT $2 FOR UPDATE", &[&board.0, &(GROUPED_ROW_LIMIT + 1)])?;
    if rows.len() > GROUPED_ROW_LIMIT as usize {
        return Err(ReorderError::TooLarge);
    }
    let mut remaining: HashSet<&str> = rows.iter().map(|r| r.get(0)).collect();
    if ids.len() != remaining.len() || !ids.iter().all(|id| remaining.remove(id.as_str())) {
        return Err(ReorderError::Conflict);
    }
    tx.execute("UPDATE board_groups g SET position=o.n-1, revision=g.revision+1 FROM unnest($2::text[]) WITH ORDINALITY o(id,n) WHERE g.board_id=$1 AND g.id=o.id", &[&board.0,&ids])?;
    tx.execute(
        "UPDATE boards SET revision=revision+1 WHERE id=$1",
        &[&board.0],
    )?;
    Ok(())
}
