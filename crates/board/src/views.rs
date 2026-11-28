//! Saved views are a named, shared-or-personal presentation of one board.
//! Quick-filter choices retain their meaning when a view is reopened.
use crate::BoardId;
use postgres::{GenericClient, error::SqlState};
use uuid::Uuid;

pub const VIEW_LIMIT: i64 = 200;
pub const FILTER_LIMIT: usize = 240;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedView {
    pub id: Uuid,
    pub name: String,
    pub filter: String,
    pub shared: bool,
    pub owner: Uuid,
    pub filters: ViewFilters,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewFilters {
    pub status_values: Vec<String>,
    pub include_empty_status: bool,
    pub group_ids: Vec<String>,
}

pub enum ViewError {
    Invalid,
    Conflict,
    Database(postgres::Error),
}

/// SavedBoardView.Name is trimmed, nonblank and at most 160 UTF-16 units.
pub fn view_name(raw: &str) -> Result<&str, ViewError> {
    let name = raw.trim();
    if name.is_empty() || name.encode_utf16().nth(160).is_some() {
        Err(ViewError::Invalid)
    } else {
        Ok(name)
    }
}

/// The filter is stored trimmed. An empty filter shows every item.
pub fn view_filter(raw: &str) -> Result<&str, ViewError> {
    let filter = raw.trim();
    if filter.encode_utf16().nth(FILTER_LIMIT).is_some() {
        Err(ViewError::Invalid)
    } else {
        Ok(filter)
    }
}

/// Shared views plus the caller's personal views, in stable name order.
pub fn list(
    db: &mut impl GenericClient,
    board: BoardId,
    account: Uuid,
) -> Result<Vec<SavedView>, ViewError> {
    db.query(
        "SELECT id, name, filter, shared, created_by, status_values, include_empty_status, group_ids FROM saved_views
         WHERE board_id=$1 AND (shared OR created_by=$2)
         ORDER BY lower(name), id LIMIT $3",
        &[&board.as_uuid(), &account, &VIEW_LIMIT],
    )
    .map_err(ViewError::Database)?
    .into_iter()
    .map(|row| {
        Ok(SavedView {
            id: row.get(0),
            name: row.get(1),
            filter: row.get(2),
            shared: row.get(3),
            owner: row.get(4),
            filters: ViewFilters {
                status_values: row.get(5),
                include_empty_status: row.get(6),
                group_ids: row.get(7),
            },
        })
    })
    .collect()
}

/// The caller has already been authorized to read this board. A personal view
/// belongs to the caller; sharing is an explicit later workflow.
pub fn create(
    db: &mut impl GenericClient,
    board: BoardId,
    id: Uuid,
    name: &str,
    filter: &str,
    owner: Uuid,
) -> Result<SavedView, ViewError> {
    create_with_filters(db, board, id, name, filter, owner, ViewFilters::default())
}

pub fn create_with_filters(
    db: &mut impl GenericClient,
    board: BoardId,
    id: Uuid,
    name: &str,
    filter: &str,
    owner: Uuid,
    mut filters: ViewFilters,
) -> Result<SavedView, ViewError> {
    let name = view_name(name)?;
    let filter = view_filter(filter)?;
    if filters.status_values.len() > 256
        || filters.group_ids.len() > 256
        || (filters.include_empty_status && !filters.status_values.is_empty())
    {
        return Err(ViewError::Invalid);
    }
    filters.status_values.sort();
    filters.status_values.dedup();
    filters.group_ids.sort();
    filters.group_ids.dedup();
    let valid: bool = db.query_one(
        "SELECT NOT EXISTS(SELECT 1 FROM unnest($1::text[]) AS wanted(value) WHERE NOT EXISTS(SELECT 1 FROM status_labels s JOIN status_columns c ON c.id=s.column_id WHERE c.board_id=$3 AND s.key=wanted.value AND s.key<>'empty'))
         AND NOT EXISTS(SELECT 1 FROM unnest($2::text[]) AS wanted(value) WHERE NOT EXISTS(SELECT 1 FROM board_groups g WHERE g.board_id=$3 AND g.id=wanted.value AND g.active))",
        &[&filters.status_values, &filters.group_ids, &board.as_uuid()],
    ).map_err(ViewError::Database)?.get(0);
    if !valid {
        return Err(ViewError::Invalid);
    }
    db.execute(
        "INSERT INTO saved_views(id, board_id, name, filter, created_by, shared, status_values, include_empty_status, group_ids)
         VALUES ($1,$2,$3,$4,$5,false,$6,$7,$8)",
        &[&id, &board.as_uuid(), &name, &filter, &owner, &filters.status_values, &filters.include_empty_status, &filters.group_ids],
    )
    .map_err(|error| match error.code() {
        Some(code) if code == &SqlState::UNIQUE_VIOLATION => ViewError::Conflict,
        _ => ViewError::Database(error),
    })?;
    Ok(SavedView {
        id,
        name: name.to_owned(),
        filter: filter.to_owned(),
        shared: false,
        owner,
        filters,
    })
}
