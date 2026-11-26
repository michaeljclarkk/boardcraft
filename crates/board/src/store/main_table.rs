use super::{
    BoardId, GROUPED_ROW_LIMIT, ReorderError, reorder::lock_board,
    selection_move::check_grid_budget,
};
use postgres::Transaction;
use std::collections::HashSet;

/// Caller holds selected-board write authorization until commit. The board
/// lock serializes both the implicit first revision and changes to membership.
pub fn set_main_table_fold(
    tx: &mut Transaction<'_>,
    board: BoardId,
    baseline: i64,
    ids: &[String],
    collapsed: bool,
) -> Result<i64, ReorderError> {
    if ids.len() > GROUPED_ROW_LIMIT as usize {
        return Err(ReorderError::TooLarge);
    }
    let mut unique = HashSet::new();
    if baseline < 1
        || ids.is_empty()
        || ids
            .iter()
            .any(|id| id.trim().is_empty() || id.contains('\0') || !unique.insert(id))
    {
        return Err(ReorderError::Invalid);
    }
    lock_board(tx, board)?;
    let current = tx
        .query_opt(
            "SELECT revision FROM main_tables WHERE board_id=$1 FOR UPDATE",
            &[&board.0],
        )?
        .map_or(1i64, |r| r.get(0));
    if current != baseline {
        return Err(ReorderError::Conflict);
    }
    check_grid_budget(tx, board)?;
    let targets = tx.query("SELECT id FROM board_groups WHERE board_id=$1 AND active AND NOT deleted AND id=ANY($2) ORDER BY id FOR SHARE", &[&board.0, &ids])?;
    if targets.len() != ids.len() {
        return Err(ReorderError::Missing);
    }
    let revision = tx
        .query_one(
            "INSERT INTO main_tables(board_id,revision) VALUES($1,2)
         ON CONFLICT(board_id) DO UPDATE SET revision=main_tables.revision+1 RETURNING revision",
            &[&board.0],
        )?
        .get(0);
    if collapsed {
        tx.execute(
            "INSERT INTO main_table_folds(board_id,group_id) SELECT $1,unnest($2::text[]) ON CONFLICT DO NOTHING",
            &[&board.0, &ids],
        )?;
    } else {
        tx.execute(
            "DELETE FROM main_table_folds WHERE board_id=$1 AND group_id=ANY($2)",
            &[&board.0, &ids],
        )?;
    }
    tx.execute(
        "UPDATE boards SET revision=revision+1 WHERE id=$1",
        &[&board.0],
    )?;
    Ok(revision)
}
