use super::{BoardId, ReorderError, reorder::lock_board, selection_move::check_grid_budget};
use postgres::Transaction;
use uuid::Uuid;

fn stored_name<'a>(raw: &'a str, colour: &str) -> Result<&'a str, ReorderError> {
    let name = raw.trim();
    if name.contains('\0')
        || !(1..=160).contains(&name.encode_utf16().count())
        || !colour
            .strip_prefix('#')
            .is_some_and(|hex| hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(ReorderError::Invalid);
    }
    Ok(name)
}

/// Explicit Save of an existing active group, including an unchanged draft.
pub fn edit_group(
    tx: &mut Transaction<'_>,
    board: BoardId,
    id: &str,
    board_revision: i64,
    group_revision: i64,
    raw_name: &str,
    colour: &str,
) -> Result<(), ReorderError> {
    let name = stored_name(raw_name, colour)?;
    if board_revision < 1 || group_revision < 1 {
        return Err(ReorderError::Invalid);
    }
    if lock_board(tx, board)? != board_revision {
        return Err(ReorderError::Conflict);
    }
    let current: i64 = tx.query_opt("SELECT revision FROM board_groups WHERE board_id=$1 AND id=$2 AND active AND NOT deleted FOR UPDATE", &[&board.0, &id])?.ok_or(ReorderError::Missing)?.get(0);
    if current != group_revision {
        return Err(ReorderError::Conflict);
    }
    tx.execute(
        "UPDATE board_groups SET name=$3,colour=$4,revision=revision+1 WHERE board_id=$1 AND id=$2",
        &[&board.0, &id, &name, &colour],
    )?;
    tx.execute(
        "UPDATE boards SET revision=revision+1 WHERE id=$1",
        &[&board.0],
    )?;
    Ok(())
}

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
    let name = stored_name(raw_name, colour)?;
    let uuid = id.strip_prefix("g-").and_then(|s| Uuid::parse_str(s).ok());
    if !uuid.is_some_and(|u| !u.is_nil() && id == format!("g-{u}")) || baseline < 1 {
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
