use super::{
    BoardId, ItemId, ReorderError, reorder::lock_board, selection_move::check_grid_budget,
};
use postgres::{Transaction, types::ToSql};
use uuid::Uuid;

pub struct ArchivedRoot {
    pub id: Uuid,
    pub title: String,
    pub group_id: String,
    pub group_name: String,
    pub revision: i64,
    pub state: i32,
    pub group_active: bool,
}

pub struct ArchivePage {
    pub revision: i64,
    pub total: i64,
    pub items: Vec<ArchivedRoot>,
}

/// Keep the board revision even for an empty archive. Deleted (own or inherited)
/// takes precedence over Archived; Staging by itself is not a recovery state.
pub fn archive_page(tx: &mut Transaction<'_>, board: BoardId) -> Result<ArchivePage, ReorderError> {
    let rows = tx.query(
        "SELECT b.revision,p.id,p.title,p.group_id,p.name,p.revision,p.state,p.active,p.total
        FROM boards b LEFT JOIN LATERAL (
        SELECT i.id,i.title,i.group_id,g.name,i.revision,i.state,g.active,count(*) OVER () AS total,
               i.position,i.created_at
        FROM board_items i JOIN board_groups g ON (i.board_id,i.group_id)=(g.board_id,g.id)
        WHERE i.board_id=b.id AND NOT g.deleted AND i.state<>2 AND (i.state=1 OR NOT g.active)
        ORDER BY i.position,i.created_at NULLS FIRST,i.id LIMIT 100
    ) p ON true WHERE b.id=$1 ORDER BY p.position,p.created_at NULLS FIRST,p.id",
        &[&board.0],
    )?;
    let first = rows.first().ok_or(ReorderError::Missing)?;
    Ok(ArchivePage {
        revision: first.get(0),
        total: first.get::<_, Option<i64>>(8).unwrap_or(0),
        items: rows
            .into_iter()
            .filter_map(|r| {
                Some(ArchivedRoot {
                    id: r.get::<_, Option<Uuid>>(1)?,
                    title: r.get(2),
                    group_id: r.get(3),
                    group_name: r.get(4),
                    revision: r.get(5),
                    state: r.get(6),
                    group_active: r.get(7),
                })
            })
            .collect(),
    })
}

#[derive(Clone, Copy)]
pub enum Lifecycle {
    Archive,
    Restore,
}
pub struct RootCommand {
    pub action: Lifecycle,
    pub operation: Uuid,
    pub board_revision: i64,
    pub item_revision: i64,
}

/// The caller holds current Owner and selected-board grants through commit.
pub fn transition_root(
    tx: &mut Transaction<'_>,
    actor: Uuid,
    board: BoardId,
    item: ItemId,
    cmd: &RootCommand,
) -> Result<(), ReorderError> {
    use ReorderError::*;
    if cmd.operation.is_nil() {
        return Err(Invalid);
    }
    let version = lock_board(tx, board)?;
    let (kind, from, to) = match cmd.action {
        Lifecycle::Archive => ("archive", 0, 1),
        Lifecycle::Restore => ("restore", 1, 0),
    };
    let args: &[&(dyn ToSql + Sync)] = &[
        &cmd.operation,
        &actor,
        &board.0,
        &item.0,
        &cmd.board_revision,
        &cmd.item_revision,
        &kind,
    ];
    let receipt = tx.query_opt(
        "SELECT (actor_id,board_id,item_id,board_revision,item_revision,lifecycle)
            IS NOT DISTINCT FROM ($2,$3,$4,$5,$6,$7)
         FROM group_reorder_receipts WHERE operation_id=$1",
        args,
    )?;
    if let Some(row) = receipt {
        return if row.get(0) { Ok(()) } else { Err(Conflict) };
    }
    if version != cmd.board_revision {
        return Err(Conflict);
    }
    let row = tx
        .query_opt(
            "SELECT i.revision,i.state FROM board_items i JOIN board_groups g
            ON (g.board_id,g.id)=(i.board_id,i.group_id)
         WHERE i.board_id=$1 AND i.id=$2 AND g.active FOR UPDATE OF i FOR SHARE OF g",
            &[&board.0, &item.0],
        )?
        .ok_or(Missing)?;
    if row.get::<_, i64>(0) != cmd.item_revision {
        return Err(Conflict);
    }
    if row.get::<_, i32>(1) != from {
        return Err(Invalid);
    }
    if matches!(cmd.action, Lifecycle::Restore)
        && !archive_page(tx, board)?
            .items
            .iter()
            .any(|r| r.id == item.0)
    {
        return Err(Missing);
    }
    tx.execute(
        "UPDATE board_items SET state=$2,revision=revision+1 WHERE id=$1",
        &[&item.0, &to],
    )?;
    check_grid_budget(tx, board)?;
    tx.execute(
        "UPDATE boards SET revision=revision+1 WHERE id=$1",
        &[&board.0],
    )?;
    let inserted = tx.execute(
        "INSERT INTO group_reorder_receipts(operation_id,actor_id,board_id,item_id,
                                            board_revision,item_revision,lifecycle)
         VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING",
        args,
    )?;
    if inserted != 1 {
        return Err(Conflict);
    }
    Ok(())
}
