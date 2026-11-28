//! Blocking PostgreSQL operations. Call these on a worker, not the editor thread.
use postgres::{Client, Error};

use crate::{Board, BoardId, Item, ItemId, RenameError, interactive_title};

mod group_fields;
pub use group_fields::{create_group, edit_group};
mod group_lifecycle;
mod group_order;
mod main_table;
pub use group_order::reorder_groups;
pub use main_table::set_main_table_fold;
mod lifecycle;
pub use group_lifecycle::{archived_groups, transition_group};
pub use lifecycle::{
    ArchivePage, Lifecycle, RootCommand, archive_page, archive_selected_roots,
    delete_selected_roots, transition_root, trash_page,
};
mod reorder;
mod selection_move;
pub use reorder::{Reorder, ReorderError, reorder_group_item};
pub use selection_move::{SelectionMove, move_selected_items};

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
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version = 3",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0003_board_groups.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (3)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version = 4",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0004_item_creation.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (4)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version=5",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0005_item_group_move.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (5)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version=6",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0006_group_reorder.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (6)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version=7",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0007_selection_move.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (7)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version=8",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0008_root_lifecycle.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (8)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version=9",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0009_group_lifecycle.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (9)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version=10",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0010_group_colour.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (10)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version=11",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0011_main_table.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (11)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version=12",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0012_selected_root_archive.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (12)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version=13",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0013_root_trash.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (13)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version=14",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0014_saved_views.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (14)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version=15",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0015_automations.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (15)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version=16",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0016_inbox_notifications.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (16)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version=17",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0017_status_cells.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (17)", &[])?;
    }
    if tx
        .query_opt(
            "SELECT version FROM rustday_migrations WHERE version=18",
            &[],
        )?
        .is_none()
    {
        tx.batch_execute(include_str!("../migrations/0018_item_activity.sql"))?;
        tx.execute("INSERT INTO rustday_migrations VALUES (18)", &[])?;
    }
    tx.commit()
}

/// Create a board snapshot without rewriting IDs, titles or row order.
pub fn create_board(client: &mut Client, board: &Board) -> Result<(), Error> {
    let mut tx = client.transaction()?;
    tx.execute("INSERT INTO boards (id) VALUES ($1)", &[&board.id.0])?;
    tx.execute(
        "INSERT INTO status_columns(board_id) VALUES ($1)",
        &[&board.id.0],
    )?;
    tx.execute("INSERT INTO board_groups (board_id,id,name,position,active) VALUES ($1,'main','Items',0,true)", &[&board.id.0])?;
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

pub enum CreateError {
    InvalidGroup,
    InvalidTitle,
    Collision,
    Database(Error),
}

pub struct GroupItem {
    pub group_id: String,
    pub item: StoredItem,
}

// Compatible for appends and group moves, exclusive against a reorder. Always
// take this before any group/item lock; these writers do not advance the board revision.
fn lock_structure_writer(tx: &mut postgres::Transaction<'_>, board: BoardId) -> Result<(), Error> {
    tx.query_opt("SELECT id FROM boards WHERE id=$1 FOR SHARE", &[&board.0])?;
    Ok(())
}

fn group_item(row: postgres::Row, id: ItemId) -> GroupItem {
    GroupItem {
        group_id: row.get("group_id"),
        item: StoredItem {
            id,
            title: row.get("title"),
            revision: row.get("revision"),
        },
    }
}

/// The caller holds current write access through commit. Lock the group before
/// reading its tail so concurrent creators allocate distinct append positions.
pub fn create_group_item(
    tx: &mut postgres::Transaction<'_>,
    board: BoardId,
    group: &str,
    item: ItemId,
    creator: uuid::Uuid,
    draft: &str,
) -> Result<GroupItem, CreateError> {
    use CreateError::*;
    lock_structure_writer(tx, board).map_err(Database)?;
    if tx
        .query_opt(
            "SELECT id FROM board_groups WHERE board_id=$1 AND id=$2 AND active FOR UPDATE",
            &[&board.0, &group],
        )
        .map_err(Database)?
        .is_none()
    {
        return Err(InvalidGroup);
    }
    let title = interactive_title(draft).map_err(|_| InvalidTitle)?;
    let inserted = tx.query_opt("INSERT INTO board_items(id,board_id,group_id,creation_group_id,created_by,title,creation_title,position)
        SELECT $1,$2,$3,$3,$4,$5,$5,COALESCE(max(position)+1,0) FROM board_items WHERE board_id=$2 AND group_id=$3
        ON CONFLICT (id) DO NOTHING RETURNING group_id,title,revision", &[&item.0,&board.0,&group,&creator,&title]).map_err(Database)?;
    let row = match inserted {
        Some(row) => row,
        None => tx.query_opt("SELECT i.group_id,i.title,i.revision FROM board_items i JOIN board_groups g ON (g.board_id,g.id)=(i.board_id,i.group_id) WHERE i.id=$1 AND i.board_id=$2 AND creation_group_id=$3 AND created_by=$4 AND creation_title=$5 AND g.active AND i.state=0 FOR SHARE",
            &[&item.0,&board.0,&group,&creator,&title]).map_err(Database)?.ok_or(Collision)?,
    };
    Ok(group_item(row, item))
}

pub enum MoveError {
    MissingItem,
    InvalidGroup,
    Conflict,
    Database(Error),
}

/// Move the existing row within the caller's authorized transaction. The
/// group stamp, unlike equality of group IDs, detects an intervening round trip.
pub fn move_group_item(
    tx: &mut postgres::Transaction<'_>,
    board: BoardId,
    item: ItemId,
    destination: &str,
    expected: i64,
) -> Result<GroupItem, MoveError> {
    use MoveError::*;
    lock_structure_writer(tx, board).map_err(Database)?;
    let mut row = tx.query_opt("SELECT i.group_id,i.title,i.revision,i.group_revision FROM board_items i JOIN board_groups g ON (g.board_id,g.id)=(i.board_id,i.group_id) WHERE i.board_id=$1 AND i.id=$2 AND g.active AND i.state=0 FOR UPDATE OF i FOR SHARE OF g", &[&board.0,&item.0]).map_err(Database)?.ok_or(MissingItem)?;
    tx.query_opt(
        "SELECT id FROM board_groups WHERE board_id=$1 AND id=$2 AND active FOR SHARE",
        &[&board.0, &destination],
    )
    .map_err(Database)?
    .ok_or(InvalidGroup)?;
    if !(row.get::<_, i64>("group_revision")..=row.get::<_, i64>("revision")).contains(&expected) {
        return Err(Conflict);
    }
    if row.get::<_, &str>("group_id") != destination {
        row = tx.query_one("UPDATE board_items SET group_id=$3,revision=revision+1,group_revision=revision+1 WHERE board_id=$1 AND id=$2 RETURNING group_id,title,revision", &[&board.0,&item.0,&destination]).map_err(Database)?;
    }
    Ok(group_item(row, item))
}

pub fn load_items(
    client: &mut impl postgres::GenericClient,
    id: BoardId,
) -> Result<Option<Vec<StoredItem>>, Error> {
    // One snapshot; the outer join distinguishes an empty board from an absent one.
    let rows = client.query(
        "SELECT i.id, i.title, i.revision FROM boards b
         LEFT JOIN board_groups g ON g.board_id=b.id AND g.active
         LEFT JOIN board_items i ON (i.board_id,i.group_id)=(g.board_id,g.id) AND i.state=0
         WHERE b.id=$1 ORDER BY g.position,g.id,i.position,i.created_at NULLS FIRST,i.id",
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

pub struct ItemGroup {
    pub collapsed: bool,
    pub revision: i64,
    pub id: String,
    pub name: String,
    pub colour: String,
    pub items: Vec<StoredItem>,
}

pub const GROUPED_ROW_LIMIT: i64 = 500;

pub enum GroupReadError {
    Missing,
    TooLarge,
    Database(Error),
}

/// One ordered snapshot. An empty group consumes one row of the projection budget.
pub fn load_groups(
    client: &mut impl postgres::GenericClient,
    board: BoardId,
) -> Result<Vec<ItemGroup>, GroupReadError> {
    load_group_snapshot(client, board).map(|(_, groups)| groups)
}

pub fn load_group_snapshot(
    client: &mut impl postgres::GenericClient,
    board: BoardId,
) -> Result<(i64, Vec<ItemGroup>), GroupReadError> {
    load_main_table(client, board).map(|(board_revision, _, groups)| (board_revision, groups))
}

pub fn load_main_table(
    client: &mut impl postgres::GenericClient,
    board: BoardId,
) -> Result<(i64, i64, Vec<ItemGroup>), GroupReadError> {
    let rows = client
        .query(
            "SELECT g.id, g.name, i.id, i.title, i.revision, b.revision, g.colour, g.revision,
                coalesce(v.revision,1), f.group_id IS NOT NULL FROM boards b
         LEFT JOIN main_tables v ON v.board_id=b.id
         LEFT JOIN board_groups g ON g.board_id=b.id AND g.active
         LEFT JOIN main_table_folds f ON (f.board_id,f.group_id)=(g.board_id,g.id)
         LEFT JOIN board_items i ON (i.board_id,i.group_id)=(g.board_id,g.id) AND i.state=0
         WHERE b.id=$1 ORDER BY g.position,g.id,i.position,i.created_at NULLS FIRST,i.id LIMIT $2",
            &[&board.0, &(GROUPED_ROW_LIMIT + 1)],
        )
        .map_err(GroupReadError::Database)?;
    if rows.is_empty() {
        return Err(GroupReadError::Missing);
    }
    if rows.len() > GROUPED_ROW_LIMIT as usize {
        return Err(GroupReadError::TooLarge);
    }
    let mut groups: Vec<ItemGroup> = Vec::new();
    let revision = rows[0].get(5);
    let view_revision = rows[0].get(8);
    for row in rows {
        let Some(id) = row.get::<_, Option<String>>(0) else {
            continue;
        };
        if groups.last().is_none_or(|g| g.id != id) {
            groups.push(ItemGroup {
                collapsed: row.get(9),
                id,
                name: row.get(1),
                colour: row.get(6),
                revision: row.get(7),
                items: Vec::new(),
            });
        }
        if let Some(id) = row.get(2) {
            groups.last_mut().unwrap().items.push(StoredItem {
                id: ItemId(id),
                title: row.get(3),
                revision: row.get(4),
            });
        }
    }
    Ok((revision, view_revision, groups))
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
            "SELECT i.title,i.revision FROM board_items i JOIN board_groups g
             ON (g.board_id,g.id)=(i.board_id,i.group_id) WHERE i.board_id=$1 AND i.id=$2 AND g.active AND i.state=0",
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
    client: &mut impl postgres::GenericClient,
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
    client: &mut impl postgres::GenericClient,
    board: BoardId,
    item: ItemId,
    condition: Condition<'_>,
    draft: &str,
) -> Result<StoredItem, SaveError> {
    save_title_with_previous(client, board, item, condition, draft).map(|(_, saved)| saved)
}

pub(crate) fn rename_with_previous(
    tx: &mut postgres::Transaction<'_>,
    board: BoardId,
    item: ItemId,
    revision: i64,
    draft: &str,
) -> Result<(String, StoredItem), SaveError> {
    save_title_with_previous(tx, board, item, Condition::KnownRevision(revision), draft)
}

fn save_title_with_previous(
    client: &mut impl postgres::GenericClient,
    board: BoardId,
    item: ItemId,
    condition: Condition<'_>,
    draft: &str,
) -> Result<(String, StoredItem), SaveError> {
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
            "SELECT i.title,i.revision FROM board_items i JOIN board_groups g
             ON (g.board_id,g.id)=(i.board_id,i.group_id) WHERE i.board_id=$1 AND i.id=$2 AND g.active AND i.state=0
             FOR UPDATE OF i FOR SHARE OF g",
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
        title: current.clone(),
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
    Ok((current, saved))
}
