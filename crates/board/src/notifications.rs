//! The inbox receipt. An automation run creates one row for the rule's owner,
//! pointing at the board and item that produced it. Reads are recipient-scoped;
//! the transport never supplies a recipient.
use crate::{BoardId, ItemId};
use postgres::GenericClient;
use uuid::Uuid;

pub const INBOX_LIMIT: i64 = 200;
pub const SEEN_LIMIT: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub id: Uuid,
    pub board_id: Uuid,
    pub item_id: Uuid,
    pub automation_id: Option<Uuid>,
    pub kind: String,
    pub title: String,
    pub message: String,
    pub item_name: String,
    pub automation_name: Option<String>,
    pub created_at: String,
    pub read_at: Option<String>,
    pub seen_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inbox {
    pub unread: i64,
    pub rows: Vec<Notification>,
}

pub enum NotificationError {
    Invalid,
    Missing,
    Database(postgres::Error),
}

/// Runs inside the accepted rename transaction, so a rolled-back rename leaves
/// neither a run nor a notification. Each enabled rule notifies its creator.
pub fn record_automation_runs(
    tx: &mut impl GenericClient,
    board: BoardId,
    item: ItemId,
    title: &str,
) -> Result<usize, postgres::Error> {
    tx.execute(
        "INSERT INTO inbox_notifications(
             id, board_id, item_id, automation_id, recipient_id, kind, title, message)
         SELECT gen_random_uuid(), a.board_id, $2, a.id, a.created_by, 'automation',
                'Automation ran', a.name || ' recorded \"' || $3 || '\"'
         FROM automations a WHERE a.board_id = $1 AND a.enabled",
        &[&board.as_uuid(), &item.as_uuid(), &title],
    )
    .map(|rows| rows as usize)
}

/// Newest first. The unread count covers the whole inbox, not just the page.
pub fn list(
    db: &mut impl GenericClient,
    recipient: Uuid,
    unread_only: bool,
) -> Result<Inbox, NotificationError> {
    let unread = db
        .query_one(
            "SELECT count(*) FROM inbox_notifications
             WHERE recipient_id = $1 AND deleted_at IS NULL AND read_at IS NULL",
            &[&recipient],
        )
        .map_err(NotificationError::Database)?
        .get(0);
    let rows = db
        .query(
            &format!(
                "{SELECT} WHERE n.recipient_id = $1 AND n.deleted_at IS NULL
                      AND (NOT $2 OR n.read_at IS NULL)
                      ORDER BY n.created_at DESC, n.id DESC LIMIT $3"
            ),
            &[&recipient, &unread_only, &INBOX_LIMIT],
        )
        .map_err(NotificationError::Database)?
        .into_iter()
        .map(read_row)
        .collect::<Result<Vec<_>, _>>()
        .map_err(NotificationError::Database)?;
    Ok(Inbox { unread, rows })
}

pub fn set_read(
    db: &mut impl GenericClient,
    recipient: Uuid,
    id: Uuid,
    read: bool,
) -> Result<Notification, NotificationError> {
    db.execute(
        "UPDATE inbox_notifications
         SET read_at = CASE WHEN $3 THEN now() ELSE NULL END
         WHERE id = $1 AND recipient_id = $2 AND deleted_at IS NULL
           AND (($3 AND read_at IS NULL) OR (NOT $3 AND read_at IS NOT NULL))",
        &[&id, &recipient, &read],
    )
    .map_err(NotificationError::Database)?;
    fetch(db, recipient, id)
}

pub fn mark_all_read(
    db: &mut impl GenericClient,
    recipient: Uuid,
) -> Result<bool, NotificationError> {
    db.execute(
        "UPDATE inbox_notifications SET read_at = now()
         WHERE recipient_id = $1 AND deleted_at IS NULL AND read_at IS NULL",
        &[&recipient],
    )
    .map_err(NotificationError::Database)?;
    Ok(true)
}

pub fn mark_seen(
    db: &mut impl GenericClient,
    recipient: Uuid,
    ids: &[Uuid],
) -> Result<bool, NotificationError> {
    if ids.is_empty() || ids.len() > SEEN_LIMIT || ids.contains(&Uuid::nil()) {
        return Err(NotificationError::Invalid);
    }
    db.execute(
        "UPDATE inbox_notifications SET seen_at = now()
         WHERE recipient_id = $1 AND id = ANY($2) AND deleted_at IS NULL AND seen_at IS NULL",
        &[&recipient, &ids],
    )
    .map_err(NotificationError::Database)?;
    Ok(true)
}

pub fn delete(
    db: &mut impl GenericClient,
    recipient: Uuid,
    id: Uuid,
) -> Result<bool, NotificationError> {
    let known = db
        .query_opt(
            "SELECT 1 FROM inbox_notifications WHERE id = $1 AND recipient_id = $2",
            &[&id, &recipient],
        )
        .map_err(NotificationError::Database)?
        .is_some();
    if !known {
        return Err(NotificationError::Missing);
    }
    db.execute(
        "UPDATE inbox_notifications SET deleted_at = now()
         WHERE id = $1 AND recipient_id = $2 AND deleted_at IS NULL",
        &[&id, &recipient],
    )
    .map_err(NotificationError::Database)?;
    Ok(true)
}

fn fetch(
    db: &mut impl GenericClient,
    recipient: Uuid,
    id: Uuid,
) -> Result<Notification, NotificationError> {
    db.query_opt(
        &format!("{SELECT} WHERE n.id = $2 AND n.recipient_id = $1 AND n.deleted_at IS NULL"),
        &[&recipient, &id],
    )
    .map_err(NotificationError::Database)?
    .map(read_row)
    .transpose()
    .map_err(NotificationError::Database)?
    .ok_or(NotificationError::Missing)
}

const SELECT: &str = "SELECT n.id, n.board_id, n.item_id, n.automation_id, n.kind,
        n.title, n.message, coalesce(i.title, ''), a.name,
        to_char(n.created_at, 'YYYY-MM-DD HH24:MI'),
        to_char(n.read_at, 'YYYY-MM-DD HH24:MI'),
        to_char(n.seen_at, 'YYYY-MM-DD HH24:MI')
 FROM inbox_notifications n
 LEFT JOIN board_items i ON i.id = n.item_id AND i.board_id = n.board_id
 LEFT JOIN automations a ON a.id = n.automation_id";

fn read_row(row: postgres::Row) -> Result<Notification, postgres::Error> {
    Ok(Notification {
        id: row.try_get(0)?,
        board_id: row.try_get(1)?,
        item_id: row.try_get(2)?,
        automation_id: row.try_get(3)?,
        kind: row.try_get(4)?,
        title: row.try_get(5)?,
        message: row.try_get(6)?,
        item_name: row.try_get(7)?,
        automation_name: row.try_get(8)?,
        created_at: row.try_get(9)?,
        read_at: row.try_get(10)?,
        seen_at: row.try_get(11)?,
    })
}
