use crate::{ApiResult, access, realtime};
use boardcraft_board::{BoardId, ItemId, history};
use boardcraft_mobile_edit_client::ActivityEntry;
use postgres::Client;
use tiny_http::{Method, Request};

pub(super) fn read(db: &mut Client, req: &Request, path: &str) -> ApiResult {
    let (board, item) = path
        .strip_prefix("/navigation/activity/")
        .and_then(|s| s.split_once('/'))
        .ok_or(404u16)?;
    if req.method() != &Method::Get {
        return Err(404);
    }
    let board = board.parse().ok().and_then(BoardId::new).ok_or(404u16)?;
    let item = item.parse().ok().and_then(ItemId::new).ok_or(404u16)?;
    let actor = access::Actor::Session(access::bearer(req)?);
    let mut tx = db.transaction().map_err(|_| 503u16)?;
    let account = actor.resolve(&mut tx)?;
    access::authorize(&mut tx, &account, board, access::Operation::SelectedRead)?;
    realtime::current_row(&mut tx, board, item)
        .map_err(|_| 503u16)?
        .ok_or(404u16)?;
    let entries = history::entries(&mut tx, board, item)
        .map_err(|_| 503u16)?
        .into_iter()
        .map(|e| ActivityEntry {
            board_id: board.as_uuid(),
            item_id: item.as_uuid(),
            actor_id: e.actor_id,
            actor_name: e.actor_name,
            occurred_at: e.occurred_at,
            field: e.field,
            before: e.before,
            after: e.after,
            revision: e.revision,
        })
        .collect::<Vec<_>>();
    tx.commit().map_err(|_| 503u16)?;
    Ok((serde_json::to_vec(&entries).map_err(|_| 500u16)?, None))
}
