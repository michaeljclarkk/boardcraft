//! The people directory for the board chrome. Only signed-in callers list it;
//! the query is applied by the account rule, never by this transport.
use crate::{ApiResult, access};
use boardcraft_account::users;
use postgres::Client;
use tiny_http::{Method, Request};

pub(super) fn route(
    db: &mut Client,
    req: &Request,
    path: &str,
    actor: Result<access::Actor, u16>,
) -> Option<ApiResult> {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    match path {
        "/navigation/users" => Some(users(db, req, query, actor)),
        _ => None,
    }
}

fn users(
    db: &mut Client,
    req: &Request,
    query: &str,
    actor: Result<access::Actor, u16>,
) -> ApiResult {
    if req.method() != &Method::Get {
        return Err(405);
    }
    let search = search_text(query)?;
    let actor = actor?;
    let mut tx = db.transaction().map_err(unavailable)?;
    actor.resolve(&mut tx)?;
    let people = users::search(&mut tx, &search).map_err(unavailable)?;
    tx.commit().map_err(unavailable)?;
    let people: Vec<_> = people
        .into_iter()
        .map(|user| {
            serde_json::json!({
                "id": user.id,
                "username": user.username,
                "email": user.email,
                "role": user.role.as_str(),
                "active": user.active,
                "teams": user.teams,
            })
        })
        .collect();
    Ok((serde_json::to_vec(&people).map_err(|_| 500u16)?, None))
}

fn unavailable(_: impl std::fmt::Debug) -> u16 {
    log::error!("list users unavailable");
    503
}

/// Accept exactly one `q` parameter. Other parameters are rejected so a stale
/// caller cannot silently search a different field.
fn search_text(query: &str) -> Result<String, u16> {
    if query.is_empty() {
        return Ok(String::new());
    }
    let mut parts = query.split('&');
    let value = parts
        .next()
        .and_then(|pair| pair.strip_prefix("q="))
        .ok_or(400u16)?;
    if parts.next().is_some() {
        return Err(400);
    }
    percent_encoding::percent_decode_str(value)
        .decode_utf8()
        .map(|text| text.into_owned())
        .map_err(|_| 400)
}
