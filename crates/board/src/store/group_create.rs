use super::{BoardId, ReorderError, reorder::lock_board, selection_move::check_grid_budget};
use postgres::Transaction;
use uuid::Uuid;

/// Under current Owner access, create an empty physical group. Its board-local
/// key doubles as its replay identity; even a stale retry cannot rename it.
pub fn create_group(
    tx: &mut Transaction<'_>,
    board: BoardId,
    id: &str,
    baseline: i64,
    raw_name: &str,
    colour: &str,
) -> Result<(), ReorderError> {
    let name = raw_name.trim();
    let uuid = id.strip_prefix("g-").and_then(|s| Uuid::parse_str(s).ok());
    if !uuid.is_some_and(|u| !u.is_nil() && id == format!("g-{u}"))
        || baseline < 1
        || name.contains('\0')
        || !(1..=160).contains(&name.encode_utf16().count())
        || !colour
            .strip_prefix('#')
            .is_some_and(|hex| hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(ReorderError::Invalid);
    }
    let revision = lock_board(tx, board)?;
    if tx
        .query_opt(
            "SELECT 1 FROM board_groups WHERE board_id=$1 AND id=$2",
            &[&board.0, &id],
        )?
        .is_some()
    {
        return Ok(());
    }
    if revision != baseline {
        return Err(ReorderError::Conflict);
    }
    tx.execute(
        "INSERT INTO board_groups(board_id,id,name,position,active,colour)
        SELECT $1,$2,$3,count(*),true,$4 FROM board_groups WHERE board_id=$1 AND active",
        &[&board.0, &id, &name, &colour],
    )?;
    check_grid_budget(tx, board)?;
    tx.execute(
        "UPDATE boards SET revision=revision+1 WHERE id=$1",
        &[&board.0],
    )?;
    Ok(())
}
