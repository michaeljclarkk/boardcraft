use super::{
    BoardId, Lifecycle, ReorderError, reorder::lock_board, selection_move::check_grid_budget,
};
use postgres::Transaction;

pub struct ArchivedGroup {
    pub id: String,
    pub name: String,
    pub position: i64,
    pub revision: i64,
}

pub struct GroupPage {
    pub revision: i64,
    pub groups: Vec<ArchivedGroup>,
    pub next_offset: Option<i64>,
}

/// Pin the board revision through the accompanying root inventory read. A
/// continuation from before a lifecycle change must restart at the first page.
pub fn archived_groups(
    tx: &mut Transaction<'_>,
    board: BoardId,
    cursor: Option<(i64, i64)>,
) -> Result<GroupPage, ReorderError> {
    let revision: i64 = tx
        .query_opt(
            "SELECT revision FROM boards WHERE id=$1 FOR SHARE",
            &[&board.0],
        )?
        .ok_or(ReorderError::Missing)?
        .get(0);
    let offset = match cursor {
        Some((v, _)) if v != revision => return Err(ReorderError::Conflict),
        Some((_, n)) if n < 0 => return Err(ReorderError::Invalid),
        Some((_, n)) => n,
        None => 0,
    };
    let rows = tx.query("SELECT id,name,position,revision FROM board_groups WHERE board_id=$1 AND NOT active AND NOT deleted ORDER BY position,id LIMIT 101 OFFSET $2", &[&board.0,&offset])?;
    Ok(GroupPage {
        revision,
        next_offset: (rows.len() > 100).then(|| offset + 100),
        groups: rows
            .into_iter()
            .take(100)
            .map(|r| ArchivedGroup {
                id: r.get(0),
                name: r.get(1),
                position: r.get(2),
                revision: r.get(3),
            })
            .collect(),
    })
}

/// Both Undo and durable Restore use this board-baselined operation. Children
/// retain their own states; visibility follows the existing group predicate.
pub fn transition_group(
    tx: &mut Transaction<'_>,
    board: BoardId,
    group: &str,
    action: Lifecycle,
    baseline: i64,
) -> Result<(), ReorderError> {
    if !matches!(action, Lifecycle::Archive | Lifecycle::Restore) {
        return Err(ReorderError::Invalid);
    }
    if lock_board(tx, board)? != baseline {
        return Err(ReorderError::Conflict);
    }
    let row = tx
        .query_opt(
            "SELECT active,deleted FROM board_groups WHERE board_id=$1 AND id=$2 FOR UPDATE",
            &[&board.0, &group],
        )?
        .ok_or(ReorderError::Missing)?;
    let active = matches!(action, Lifecycle::Restore);
    if row.get::<_, bool>(1) || row.get::<_, bool>(0) == active {
        return Err(ReorderError::Invalid);
    }
    tx.execute(
        "UPDATE board_groups SET active=$3,revision=revision+1 WHERE board_id=$1 AND id=$2",
        &[&board.0, &group, &active],
    )?;
    if active {
        check_grid_budget(tx, board)?;
    }
    tx.execute(
        "UPDATE boards SET revision=revision+1 WHERE id=$1",
        &[&board.0],
    )?;
    Ok(())
}
