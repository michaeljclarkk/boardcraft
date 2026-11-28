//! Saved views are a named, shared-or-personal presentation of one board.
//! The first slice stores a case-insensitive item title filter.
use crate::BoardId;
use postgres::{GenericClient, error::SqlState};
use uuid::Uuid;

pub const VIEW_LIMIT: i64 = 200;
pub const FILTER_LIMIT: usize = 320;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedView {
    pub id: Uuid,
    pub name: String,
    pub filter: String,
    pub shared: bool,
    pub owner: Uuid,
}

pub enum ViewError {
    Invalid,
    Conflict,
    Database(postgres::Error),
}

/// A view name follows the interactive title rule: trimmed, nonblank and at
/// most 255 UTF-16 units.
pub fn view_name(raw: &str) -> Result<&str, ViewError> {
    let name = raw.trim();
    if name.is_empty() || name.encode_utf16().nth(255).is_some() {
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
        "SELECT id, name, filter, shared, created_by FROM saved_views
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
    let name = view_name(name)?;
    let filter = view_filter(filter)?;
    db.execute(
        "INSERT INTO saved_views(id, board_id, name, filter, created_by, shared)
         VALUES ($1,$2,$3,$4,$5,false)",
        &[&id, &board.as_uuid(), &name, &filter, &owner],
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
    })
}
