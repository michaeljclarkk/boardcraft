//! The first automation rule: when an item title is committed, an enabled
//! automation on that board records the item and its accepted title.
use crate::{BoardId, ItemId};
use postgres::{GenericClient, error::SqlState};
use uuid::Uuid;

pub const AUTOMATION_LIMIT: i64 = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Automation {
    pub id: Uuid,
    pub name: String,
    pub trigger: String,
    pub action: String,
    pub enabled: bool,
    pub last_item_id: Option<Uuid>,
    pub last_item_title: Option<String>,
    pub last_ran_at: Option<String>,
}

pub enum AutomationError {
    Invalid,
    Conflict,
    Database(postgres::Error),
}

pub fn automation_name(raw: &str) -> Result<&str, AutomationError> {
    let name = raw.trim();
    if name.is_empty() || name.encode_utf16().nth(255).is_some() {
        Err(AutomationError::Invalid)
    } else {
        Ok(name)
    }
}

/// Shared and board-wide: any caller with read access may list rules.
pub fn list(
    db: &mut impl GenericClient,
    board: BoardId,
) -> Result<Vec<Automation>, AutomationError> {
    db.query(
        "SELECT a.id, a.name, a.trigger, a.action, a.enabled,
                r.item_id, r.item_title, r.ran_at::text
         FROM automations a
         LEFT JOIN LATERAL (
             SELECT item_id, item_title, ran_at FROM automation_runs
             WHERE automation_id = a.id ORDER BY ran_at DESC, id DESC LIMIT 1
         ) r ON true
         WHERE a.board_id = $1
         ORDER BY lower(a.name), a.id LIMIT $2",
        &[&board.as_uuid(), &AUTOMATION_LIMIT],
    )
    .map_err(AutomationError::Database)?
    .into_iter()
    .map(|row| {
        Ok(Automation {
            id: row.get(0),
            name: row.get(1),
            trigger: row.get(2),
            action: row.get(3),
            enabled: row.get(4),
            last_item_id: row.get(5),
            last_item_title: row.get(6),
            last_ran_at: row.get(7),
        })
    })
    .collect()
}

pub fn create(
    db: &mut impl GenericClient,
    board: BoardId,
    id: Uuid,
    name: &str,
    owner: Uuid,
) -> Result<Automation, AutomationError> {
    let name = automation_name(name)?;
    db.execute(
        "INSERT INTO automations(id, board_id, name, trigger, action, enabled, created_by)
         VALUES ($1,$2,$3,'item_renamed','record',true,$4)",
        &[&id, &board.as_uuid(), &name, &owner],
    )
    .map_err(|error| match error.code() {
        Some(code) if code == &SqlState::UNIQUE_VIOLATION => AutomationError::Conflict,
        _ => AutomationError::Database(error),
    })?;
    Ok(Automation {
        id,
        name: name.to_owned(),
        trigger: "item_renamed".into(),
        action: "record".into(),
        enabled: true,
        last_item_id: None,
        last_item_title: None,
        last_ran_at: None,
    })
}

/// Run the board's enabled rules inside the caller's rename transaction. The
/// accepted title is stamped, so a rolled-back rename leaves no run.
pub fn run_renamed(
    tx: &mut impl GenericClient,
    board: BoardId,
    item: ItemId,
    title: &str,
) -> Result<usize, postgres::Error> {
    tx.query(
        "INSERT INTO automation_runs(id, automation_id, board_id, item_id, item_title)
         SELECT gen_random_uuid(), a.id, a.board_id, $2, $3
         FROM automations a WHERE a.board_id = $1 AND a.enabled
         RETURNING id",
        &[&board.as_uuid(), &item.as_uuid(), &title],
    )
    .map(|rows| rows.len())
}
