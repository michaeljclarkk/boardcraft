use crate::{ApiResult, access, read_json};
use boardcraft_board::{BoardId, ItemId, columns};
use boardcraft_mobile_edit_client::{
    StatusCell, StatusChoice, StatusEdit, StatusLabelEdit, StatusLabels, StatusTable,
};
use postgres::Client;
use tiny_http::{Method, Request};
use uuid::Uuid;

pub(super) fn route(
    db: &mut Client,
    req: &mut Request,
    path: &str,
    actor: Result<access::Actor, u16>,
) -> ApiResult {
    let actor = actor?;
    if path.contains("/labels") {
        return labels_route(db, req, path, actor);
    }
    let mut path = path.trim_start_matches("/navigation/status/").split('/');
    let board = path
        .next()
        .and_then(|id| id.parse().ok())
        .and_then(BoardId::new)
        .ok_or(404u16)?;
    let write = match (req.method(), path.next(), path.next(), path.next()) {
        (&Method::Get, None, None, None) => None,
        (&Method::Put, Some(c), Some(i), None) => Some((
            c.parse::<Uuid>().map_err(|_| 404u16)?,
            i.parse().ok().and_then(ItemId::new).ok_or(404u16)?,
            read_json::<StatusEdit>(req)?,
        )),
        _ => return Err(404),
    };
    let mut tx = db.transaction().map_err(|_| 503u16)?;
    let account = actor.resolve(&mut tx)?;
    let op = if write.is_some() {
        access::Operation::SelectedWrite
    } else {
        access::Operation::SelectedRead
    };
    access::authorize(&mut tx, &account, board, op)?;
    let body = if let Some((col, item, draft)) = write {
        let (row, previous) = columns::set_status_with_previous(
            &mut tx,
            board,
            col,
            item,
            draft.revision,
            draft.value,
        )
        .map_err(status)?;
        if previous != row.value {
            boardcraft_board::automation::apply(
                &mut tx,
                board,
                item,
                boardcraft_board::automation::Event::column_changed(
                    account.id,
                    &col.to_string(),
                    previous,
                    row.value.clone(),
                    Some(format!("status:{}", row.revision)),
                ),
            )
            .map_err(|error| match error {
                boardcraft_board::automation::EngineError::Rejected(message) => {
                    log::warn!("automation rejected: {message}");
                    422u16
                }
                boardcraft_board::automation::EngineError::Database(error) => {
                    log::error!("automation execution sqlstate={:?}", error.code());
                    500u16
                }
            })?;
        }
        serde_json::to_vec(&cell(row))
    } else {
        let t = columns::read_status(&mut tx, board).map_err(status)?;
        serde_json::to_vec(&StatusTable {
            column_id: t.id,
            title: t.title,
            choices: t
                .choices
                .into_iter()
                .map(|c| StatusChoice {
                    key: c.key,
                    label: c.label,
                    color: c.color,
                })
                .collect(),
            cells: t.cells.into_iter().map(cell).collect(),
        })
    }
    .map_err(|_| 503u16)?;
    tx.commit().map_err(|_| 503u16)?;
    Ok((body, None))
}

fn labels_route(db: &mut Client, req: &mut Request, path: &str, actor: access::Actor) -> ApiResult {
    let parts: Vec<_> = path
        .trim_start_matches("/navigation/status/")
        .split('/')
        .collect();
    let [board, column, "labels", ..] = parts.as_slice() else {
        return Err(404);
    };
    let board = board.parse().ok().and_then(BoardId::new).ok_or(404u16)?;
    let column = column.parse::<Uuid>().map_err(|_| 404u16)?;
    let write = match (req.method(), parts.len()) {
        (&Method::Get, 3) => None,
        (&Method::Put, 4) => Some((parts[3], read_json::<StatusLabelEdit>(req)?)),
        _ => return Err(404),
    };
    let mut tx = db.transaction().map_err(|_| 503u16)?;
    let account = actor.resolve(&mut tx)?;
    access::authorize(
        &mut tx,
        &account,
        board,
        if write.is_some() {
            access::Operation::SelectedOwner
        } else {
            access::Operation::SelectedRead
        },
    )?;
    let bytes = if let Some((key, patch)) = write {
        let saved = columns::save_status_label(
            &mut tx,
            board,
            column,
            key,
            patch.label.as_deref(),
            patch.color.as_deref(),
        )
        .map_err(status)?;
        serde_json::to_vec(&StatusChoice {
            key: saved.key,
            label: saved.label,
            color: saved.color,
        })
    } else {
        let choices = columns::read_status_labels(&mut tx, board, column)
            .map_err(status)?
            .into_iter()
            .map(|c| StatusChoice {
                key: c.key,
                label: c.label,
                color: c.color,
            })
            .collect();
        serde_json::to_vec(&StatusLabels {
            actor_id: account.id,
            column_id: column,
            choices,
            can_edit: account.role == boardcraft_account::Role::Owner,
        })
    }
    .map_err(|_| 503u16)?;
    tx.commit().map_err(|_| 503u16)?;
    Ok((bytes, None))
}
fn cell(c: columns::Cell) -> StatusCell {
    StatusCell {
        item_id: c.item,
        revision: c.revision,
        value: c.value,
    }
}
fn status(e: columns::Error) -> u16 {
    match e {
        columns::Error::Missing => 404,
        columns::Error::Invalid => 422,
        columns::Error::Conflict => 409,
        columns::Error::Store(e) => {
            log::error!("status cell database error: {e}");
            503
        }
    }
}
