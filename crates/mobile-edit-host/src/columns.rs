use crate::{ApiResult, access, read_json};
use boardcraft_board::{BoardId, ItemId, columns};
use boardcraft_mobile_edit_client::{StatusCell, StatusChoice, StatusEdit, StatusTable};
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
        let row = columns::set_status(&mut tx, board, col, item, draft.revision, draft.value)
            .map_err(status)?;
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
