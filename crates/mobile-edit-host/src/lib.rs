//! Single-board host for a disposable database. No database configuration crosses HTTP.
mod access;
use std::{io::Read, sync::Arc, thread};

use boardcraft_board::{
    BoardId, ItemId, RenameError,
    store::{self, SaveError, StoredItem},
};
use boardcraft_mobile_edit_client::{Credentials, NameEdit, SavedItem};
use postgres::{Client, Config, NoTls};
use tiny_http::{Header, Method, Request, Response, Server};
use uuid::Uuid;

pub struct Host {
    server: Arc<Server>,
    worker: Option<thread::JoinHandle<()>>,
    pub port: u16,
}

impl Host {
    pub fn start(config: &Config, board: BoardId) -> Result<Self, &'static str> {
        if config.get_dbname() != Some("rustday_test") {
            return Err("expected explicit rustday_test database");
        }
        let mut db = config
            .connect(NoTls)
            .map_err(|_| "database connection failed")?;
        store::migrate(&mut db).map_err(|_| "board migration failed")?;
        boardcraft_account::migrate(config).map_err(|_| "account migration failed")?;
        db.batch_execute(include_str!("../migrations/0001_private_board_access.sql"))
            .map_err(|_| "board access migration failed")?;
        let server = Arc::new(
            Server::http((std::net::Ipv4Addr::LOCALHOST, 0)).map_err(|_| "loopback bind failed")?,
        );
        let port = server.server_addr().to_ip().expect("TCP listener").port();
        let listener = server.clone();
        let config = config.clone();
        let worker = thread::spawn(move || {
            while let Ok(mut req) = listener.recv() {
                let result = if req.url() == "/session" {
                    session(&config, &mut db, &mut req)
                } else {
                    handle(&mut db, board, &mut req)
                };
                let response = match result {
                    Ok(body) => Response::from_data(body).with_header(
                        Header::from_bytes("Content-Type", "application/json").unwrap(),
                    ),
                    Err(401) => Response::from_data(Vec::new())
                        .with_status_code(401)
                        .with_header(Header::from_bytes("WWW-Authenticate", "Bearer").unwrap()),
                    Err(status) => Response::from_data(Vec::new()).with_status_code(status),
                }
                .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap());
                // Losing the response does not undo a commit. The caller can read/replay.
                let _ = req.respond(response);
            }
        });
        Ok(Self {
            server,
            worker: Some(worker),
            port,
        })
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.server.unblock();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn wire(row: StoredItem) -> SavedItem {
    SavedItem {
        id: row.id.as_uuid(),
        title: row.title,
        revision: row.revision,
    }
}

fn session(config: &Config, db: &mut Client, req: &mut Request) -> Result<Vec<u8>, u16> {
    match req.method() {
        Method::Post => {
            if !req.headers().iter().any(|h| {
                h.field.equiv("Content-Type")
                    && h.value.as_str().split(';').next() == Some("application/json")
            }) {
                return Err(415);
            }
            let credentials: Credentials =
                serde_json::from_slice(&read_body(req)?).map_err(|_| 400u16)?;
            let token =
                boardcraft_account::sign_in(config, &credentials.identifier, &credentials.password)
                    .map_err(access::account_status)?;
            Ok(serde_json::to_vec(token.encoded()).expect("serializable token"))
        }
        Method::Get => {
            boardcraft_account::resolve_on(db, &access::bearer(req)?)
                .map_err(access::account_status)?;
            Ok(b"null".to_vec())
        }
        _ => Err(405),
    }
}

fn handle(db: &mut Client, board: BoardId, req: &mut Request) -> Result<Vec<u8>, u16> {
    let path: Vec<_> = req.url().split('/').collect();
    let ["", "boards", requested, route @ ..] = path.as_slice() else {
        return Err(404);
    };
    let requested = Uuid::parse_str(requested).map_err(|_| 404u16)?;
    let target = match (req.method(), route) {
        (Method::Get, []) => None,
        (Method::Put, ["items", id, "name"]) => Some(
            Uuid::parse_str(id)
                .ok()
                .and_then(ItemId::new)
                .ok_or(404u16)?,
        ),
        _ => return Err(404),
    };
    let token = access::bearer(req)?;
    // Buffer bounded input before acquiring database locks. Do not return draft
    // validation or board data before the caller has been authorized.
    let rename = target.map(|item| (item, read_edit(req)));
    let mut tx = db.transaction().map_err(|_| {
        log::error!("begin board request failed board={requested}");
        503u16
    })?;
    let account = access::resolve(&mut tx, &token)?;
    if requested != board.as_uuid() {
        return Err(404);
    }
    access::authorize(&mut tx, account, board, rename.is_some())?;
    let body = match rename {
        None => {
            let rows = store::load_items(&mut tx, board)
                .map_err(|error| {
                    log::error!(
                        "load_items board={} sqlstate={:?}",
                        board.as_uuid(),
                        error.code()
                    );
                    500u16
                })?
                .ok_or(404u16)?;
            let items: Vec<_> = rows.into_iter().map(wire).collect();
            serde_json::to_vec(&items).expect("serializable item list")
        }
        Some((item, edit)) => {
            let edit = edit?;
            let saved =
                store::rename_item_at_revision(&mut tx, board, item, edit.revision, &edit.title)
                    .map_err(|error| match error {
                        SaveError::Rejected(RenameError::MissingItem(_)) => 404u16,
                        SaveError::Rejected(_) => 422,
                        SaveError::Conflict { .. } => 409,
                        SaveError::Database(_) => 500,
                    })?;
            serde_json::to_vec(&wire(saved)).expect("serializable item")
        }
    };
    tx.commit().map_err(|_| {
        log::error!("board request commit failed board={requested}");
        500u16
    })?;
    Ok(body)
}

fn read_edit(req: &mut Request) -> Result<NameEdit, u16> {
    serde_json::from_slice(&read_body(req)?).map_err(|_| 400)
}

fn read_body(req: &mut Request) -> Result<Vec<u8>, u16> {
    const MAX_BODY: u64 = 64 * 1024;
    let mut body = Vec::new();
    req.as_reader()
        .take(MAX_BODY + 1)
        .read_to_end(&mut body)
        .map_err(|_| 400u16)?;
    if body.len() as u64 > MAX_BODY {
        return Err(413);
    }
    Ok(body)
}
