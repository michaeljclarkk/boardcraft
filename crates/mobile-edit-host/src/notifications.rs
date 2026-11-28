//! Recipient-scoped inbox routes. The recipient is always the resolved session;
//! a request cannot name another person's inbox or change its rows.
use crate::{ApiResult, access, read_json};
use boardcraft_board::notifications;
use postgres::Client;
use tiny_http::{Method, Request};
use uuid::Uuid;

pub(super) fn route(
    db: &mut Client,
    req: &mut Request,
    path: &str,
    actor: Result<access::Actor, u16>,
) -> ApiResult {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let rest = path
        .strip_prefix("/navigation/notifications")
        .ok_or(404u16)?;
    let segments: Vec<&str> = rest
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let action = match (req.method(), segments.as_slice()) {
        (&Method::Get, []) => Action::List(unread_only(query)?),
        (&Method::Post, ["read-all"]) => {
            let _: serde_json::Value = read_json(req)?;
            Action::ReadAll
        }
        (&Method::Post, [id, "read"]) => Action::Read(uuid(id)?, read_json::<ReadDraft>(req)?.read),
        (&Method::Post, ["seen"]) => Action::Seen(read_json::<SeenDraft>(req)?.ids),
        (&Method::Delete, [id]) => Action::Delete(uuid(id)?),
        _ => return Err(404),
    };
    let actor = actor?;
    let mut tx = db.transaction().map_err(unavailable)?;
    let account = actor.resolve(&mut tx)?;
    let body = match action {
        Action::List(unread_only) => {
            let inbox = notifications::list(&mut tx, account.id, unread_only).map_err(status)?;
            let names = board_names(&mut tx, &inbox.rows)?;
            let rows: Vec<_> = inbox
                .rows
                .into_iter()
                .map(|row| {
                    let name = names.get(&row.board_id).map_or("", String::as_str);
                    wire(row, name)
                })
                .collect();
            serde_json::to_vec(&serde_json::json!({
                "unread": inbox.unread,
                "notifications": rows,
            }))
            .expect("serializable inbox")
        }
        Action::Read(id, read) => {
            let row = notifications::set_read(&mut tx, account.id, id, read).map_err(status)?;
            let names = board_names(&mut tx, std::slice::from_ref(&row))?;
            let name = names.get(&row.board_id).map_or("", String::as_str);
            serde_json::to_vec(&wire(row, name)).expect("serializable notification")
        }
        Action::Seen(ids) => {
            notifications::mark_seen(&mut tx, account.id, &ids).map_err(status)?;
            b"true".to_vec()
        }
        Action::ReadAll => {
            notifications::mark_all_read(&mut tx, account.id).map_err(status)?;
            b"true".to_vec()
        }
        Action::Delete(id) => {
            notifications::delete(&mut tx, account.id, id).map_err(status)?;
            b"true".to_vec()
        }
    };
    tx.commit().map_err(unavailable)?;
    Ok((body, None))
}

enum Action {
    List(bool),
    Read(Uuid, bool),
    Seen(Vec<Uuid>),
    ReadAll,
    Delete(Uuid),
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadDraft {
    read: bool,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SeenDraft {
    ids: Vec<Uuid>,
}

fn wire(row: notifications::Notification, board_name: &str) -> serde_json::Value {
    serde_json::json!({
        "id": row.id,
        "board_id": row.board_id,
        "item_id": row.item_id,
        "automation_id": row.automation_id,
        "kind": row.kind,
        "title": row.title,
        "message": row.message,
        "board_name": board_name,
        "item_name": row.item_name,
        "automation_name": row.automation_name,
        "created_at": row.created_at,
        "read_at": row.read_at,
        "seen_at": row.seen_at,
    })
}

/// Board display names live in the host's access catalog, not the board layer.
fn board_names(
    tx: &mut postgres::Transaction<'_>,
    rows: &[notifications::Notification],
) -> Result<std::collections::HashMap<Uuid, String>, u16> {
    let ids: Vec<Uuid> = rows.iter().map(|row| row.board_id).collect();
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    Ok(tx
        .query(
            "SELECT board_id, name FROM board_policies WHERE board_id = ANY($1)",
            &[&ids],
        )
        .map_err(unavailable)?
        .into_iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect())
}

fn uuid(raw: &str) -> Result<Uuid, u16> {
    Uuid::parse_str(raw)
        .ok()
        .and_then(|id| (!id.is_nil()).then_some(id))
        .ok_or(400u16)
}

/// Accept exactly one optional `unread=1`; a stale caller cannot silently
/// filter a different field.
fn unread_only(query: &str) -> Result<bool, u16> {
    match query {
        "" => Ok(false),
        "unread=1" => Ok(true),
        _ => Err(400),
    }
}

fn unavailable(error: postgres::Error) -> u16 {
    log::error!("notification inbox unavailable sqlstate={:?}", error.code());
    503
}

fn status(error: notifications::NotificationError) -> u16 {
    match error {
        notifications::NotificationError::Invalid => 422,
        notifications::NotificationError::Missing => 404,
        notifications::NotificationError::Database(error) => {
            log::error!("notification inbox sqlstate={:?}", error.code());
            500
        }
    }
}
