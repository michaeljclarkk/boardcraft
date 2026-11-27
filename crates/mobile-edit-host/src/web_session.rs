//! Browser-only transport. A cookie travels across loopback ports; by itself it
//! cannot select a bearer. The second random half stays at the issuing origin.
use std::collections::HashMap;

use boardcraft_account::Token;
use tiny_http::{Header, Method, Request};
use uuid::Uuid;

use crate::{ApiResult, Config, access, login, navigation_boards};

pub(super) const PREFIX: &str = "/web/api";
const BINDING: &str = "Boardcraft-Web";

pub(super) fn one<'a>(req: &'a Request, field: &'static str) -> Result<Option<&'a str>, u16> {
    let mut values = req.headers().iter().filter(|h| h.field.equiv(field));
    let value = values.next().map(|h| h.value.as_str());
    if values.next().is_some() {
        Err(400)
    } else {
        Ok(value)
    }
}

pub(super) struct Sessions {
    authority: String,
    origin: String,
    cookie: String,
    pairs: HashMap<(Uuid, Uuid), Token>,
}

impl Sessions {
    pub(super) fn new(port: u16) -> Self {
        let authority = format!("127.0.0.1:{port}");
        Self {
            origin: format!("http://{authority}"),
            authority,
            cookie: format!("boardcraft-{}", Uuid::new_v4()),
            pairs: HashMap::new(),
        }
    }

    pub(super) fn authority_matches(&self, req: &Request) -> bool {
        one(req, "Host") == Ok(Some(self.authority.as_str()))
    }

    pub(super) fn dispatch(
        &mut self,
        req: &mut Request,
        path: &str,
        config: &Config,
        db: &mut postgres::Client,
        board: crate::BoardId,
        cookie_out: &mut Option<Header>,
    ) -> ApiResult {
        let origin = one(req, "Origin")?;
        if origin.is_some_and(|v| v != self.origin)
            || (*req.method() != Method::Get && origin.is_none())
            || one(req, "Sec-Fetch-Site")?.is_some_and(|v| v != "same-origin")
            || one(req, "Authorization")?.is_some()
        {
            return Err(403);
        }
        let binding = one(req, BINDING)?.ok_or(401u16)?;
        if path == "/session" && *req.method() == Method::Post {
            if binding != "login" {
                return Err(403);
            }
            if self.pairs.len() >= 256 {
                return Err(503);
            }
            let token = login(config, req)?;
            let pair = (Uuid::new_v4(), Uuid::new_v4());
            self.pairs.insert(pair, token);
            *cookie_out = Some(self.set_cookie(&pair.0.to_string()));
            return Ok((serde_json::to_vec(&pair.1).unwrap(), None));
        }
        let mut handles = one(req, "Cookie")?
            .unwrap_or("")
            .split(';')
            .filter_map(|part| part.trim().split_once('='))
            .filter(|(key, _)| *key == self.cookie);
        let parse = |v: &str| Uuid::parse_str(v).map_err(|_| 401u16);
        let pair = (parse(handles.next().ok_or(401u16)?.1)?, parse(binding)?);
        if handles.next().is_some() {
            return Err(401);
        }
        let token = self.pairs.get(&pair).ok_or(401u16)?;
        let result = match (req.method(), path) {
            (&Method::Get, "/session") => {
                crate::resolve_session(db, token).map(|body| (body, None))
            }
            (&Method::Post, "/session/logout") => {
                boardcraft_account::revoke(config, token).map_err(access::account_status)?;
                self.pairs.remove(&pair);
                *cookie_out = Some(self.set_cookie(""));
                Ok((b"null".to_vec(), None))
            }
            (&Method::Get, "/navigation/boards") => {
                navigation_boards(db, req, Ok(token.clone())).map(|body| (body, None))
            }
            (_, p) if p.starts_with("/navigation/boards/") => crate::handle(
                db,
                board,
                crate::Mode::Web,
                req,
                path,
                Ok(token.clone()),
                crate::ReadShape::Grouped,
            ),
            _ => Err(404),
        };
        if matches!(result, Err(401)) {
            self.pairs.remove(&pair);
        }
        result
    }

    fn set_cookie(&self, value: &str) -> Header {
        Header::from_bytes(
            "Set-Cookie",
            format!(
                "{}={value}; Path={PREFIX}; HttpOnly; SameSite=Strict",
                self.cookie
            ),
        )
        .unwrap()
    }
}
