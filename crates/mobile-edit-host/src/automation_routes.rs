//! Authenticated automation catalog, rule and history routes.
use crate::{ApiResult, access};
use boardcraft_board::{BoardId, automation};
use postgres::Client;
use serde::Deserialize;
use tiny_http::{Method, Request};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryQuery {
    rule_id: Option<String>,
    offset: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnabledEdit {
    expected_revision: i64,
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchivedEdit {
    expected_revision: i64,
    archived: bool,
}

pub(super) fn route(
    db: &mut Client,
    req: &mut Request,
    path: &str,
    actor: Result<access::Actor, u16>,
) -> ApiResult {
    let (route, query) = path.split_once('?').unwrap_or((path, ""));
    let mut parts = route.trim_start_matches("/navigation/boards/").split('/');
    let board = parts
        .next()
        .and_then(|id| id.parse().ok())
        .and_then(BoardId::new)
        .ok_or(404u16)?;
    let tail: Vec<_> = parts.collect();
    let operation = match (req.method(), tail.as_slice()) {
        (&Method::Get, ["automation", "catalog" | "rules"]) => access::Operation::SelectedRead,
        (&Method::Post, ["automation", "history"]) => access::Operation::SelectedRead,
        (&Method::Post, ["automation", "rules"]) => access::Operation::SelectedOwner,
        (&Method::Post, ["automation", "rules", _, "enabled" | "archived"]) => {
            access::Operation::SelectedOwner
        }
        _ => return Err(404),
    };
    let actor = actor?;
    let mut tx = db.transaction().map_err(|_| 503u16)?;
    let account = actor.resolve(&mut tx)?;
    access::authorize(&mut tx, &account, board, operation)?;
    let body = match (req.method(), tail.as_slice()) {
        (&Method::Get, ["automation", "catalog"]) => {
            let catalog = automation::catalog(&mut tx, board.as_uuid()).map_err(store)?;
            serde_json::to_vec(&catalog).map_err(|_| 500u16)?
        }
        (&Method::Get, ["automation", "rules"]) => {
            let archived = flag(query, "archived").unwrap_or(false);
            let offset = number(query, "offset").unwrap_or(0);
            let rules =
                automation::list(&mut tx, board.as_uuid(), archived, offset).map_err(store)?;
            serde_json::to_vec(&rules).map_err(|_| 500u16)?
        }
        (&Method::Post, ["automation", "rules"]) => {
            let input: automation::SaveInput = crate::read_json(req)?;
            let saved =
                automation::save(&mut tx, board.as_uuid(), &input, account.id).map_err(store)?;
            serde_json::to_vec(&saved).map_err(|_| 500u16)?
        }
        (&Method::Post, ["automation", "rules", id, "enabled"]) => {
            let id = automation::parse_rule_id(id).ok_or(404u16)?;
            let edit: EnabledEdit = crate::read_json(req)?;
            let saved = automation::set_enabled(
                &mut tx,
                board.as_uuid(),
                id,
                edit.expected_revision,
                edit.enabled,
                account.id,
            )
            .map_err(store)?;
            serde_json::to_vec(&saved).map_err(|_| 500u16)?
        }
        (&Method::Post, ["automation", "rules", id, "archived"]) => {
            let id = automation::parse_rule_id(id).ok_or(404u16)?;
            let edit: ArchivedEdit = crate::read_json(req)?;
            let saved = automation::set_archived(
                &mut tx,
                board.as_uuid(),
                id,
                edit.expected_revision,
                edit.archived,
            )
            .map_err(store)?;
            serde_json::to_vec(&saved).map_err(|_| 500u16)?
        }
        (&Method::Post, ["automation", "history"]) => {
            let read: HistoryQuery = crate::read_json(req)?;
            let rule = read
                .rule_id
                .as_deref()
                .filter(|id| !id.is_empty())
                .map(|id| automation::parse_rule_id(id).ok_or(404u16))
                .transpose()?;
            let history =
                automation::history(&mut tx, board.as_uuid(), rule, read.offset).map_err(store)?;
            serde_json::to_vec(&history).map_err(|_| 500u16)?
        }
        _ => return Err(404),
    };
    tx.commit().map_err(|_| 503u16)?;
    Ok((body, None))
}

fn flag(query: &str, key: &str) -> Option<bool> {
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name == key).then_some(value == "true")
    })
}

fn number(query: &str, key: &str) -> Option<i64> {
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name == key).then(|| value.parse().ok()).flatten()
    })
}

fn store(error: automation::StoreError) -> u16 {
    match error {
        automation::StoreError::Invalid(_) | automation::StoreError::NotReady => 422,
        automation::StoreError::Conflict => 409,
        automation::StoreError::Missing => 404,
        automation::StoreError::Database(error) => {
            log::error!("automation sqlstate={:?}", error.code());
            500
        }
    }
}
