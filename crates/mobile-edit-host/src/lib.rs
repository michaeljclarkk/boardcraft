//! Single-board host for a disposable database. No database configuration crosses HTTP.
mod access;
mod preview;
use std::{io::Read, sync::Arc, thread};

use boardcraft_board::{
    BoardId, ItemId, RenameError,
    store::{self, SaveError, StoredItem},
};
use boardcraft_mobile_edit_client::{BOARD_ACCESS_HEADER, Credentials, NameEdit, SavedItem};
use postgres::{Client, Config, NoTls};
use tiny_http::{Header, Method, Request, Response, Server};
use uuid::Uuid;

pub struct Host {
    server: Arc<Server>,
    worker: Option<thread::JoinHandle<()>>,
    pub port: u16,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Scoped,
    Navigation,
    Preview,
}

impl Host {
    pub fn start(config: &Config, board: BoardId) -> Result<Self, &'static str> {
        Self::listen(config, board, Mode::Scoped)
    }

    /// Advertise the caller's visible catalog. Item requests still target only `board`.
    pub fn start_with_navigation(config: &Config, board: BoardId) -> Result<Self, &'static str> {
        Self::listen(config, board, Mode::Navigation)
    }

    /// Serve the browser preview on the same ephemeral loopback origin as the API.
    pub fn start_preview(config: &Config, board: BoardId) -> Result<Self, &'static str> {
        Self::listen(config, board, Mode::Preview)
    }

    fn listen(config: &Config, board: BoardId, mode: Mode) -> Result<Self, &'static str> {
        let navigation = mode != Mode::Scoped;
        if config.get_dbname() != Some("rustday_test") {
            return Err("expected explicit rustday_test database");
        }
        let mut db = config
            .connect(NoTls)
            .map_err(|_| "database connection failed")?;
        store::migrate(&mut db).map_err(|_| "board migration failed")?;
        boardcraft_account::migrate(config).map_err(|_| "account migration failed")?;
        access::migrate(&mut db).map_err(|_| "board access migration failed")?;
        let server = Arc::new(
            Server::http((std::net::Ipv4Addr::LOCALHOST, 0)).map_err(|_| "loopback bind failed")?,
        );
        let port = server.server_addr().to_ip().expect("TCP listener").port();
        let listener = server.clone();
        let config = config.clone();
        let worker = thread::spawn(move || {
            while let Ok(mut req) = listener.recv() {
                if mode == Mode::Preview
                    && let Some(response) = preview::asset(&req)
                {
                    let _ = req.respond(response);
                    continue;
                }
                let result = match req.url() {
                    "/session" => session(&config, &mut db, &mut req).map(|body| (body, None)),
                    "/session/logout" => logout(&config, &req).map(|body| (body, None)),
                    "/navigation/boards" if navigation => {
                        navigation_boards(&mut db, &req).map(|body| (body, None))
                    }
                    _ => handle(&mut db, board, mode, &mut req),
                };
                let response = match result {
                    Ok((body, hint)) => {
                        let mut response = Response::from_data(body).with_header(
                            Header::from_bytes("Content-Type", "application/json").unwrap(),
                        );
                        if let Some(can_edit) = hint {
                            response.add_header(
                                Header::from_bytes(
                                    BOARD_ACCESS_HEADER,
                                    if can_edit { "edit" } else { "read" },
                                )
                                .unwrap(),
                            );
                        }
                        response
                    }
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

fn logout(config: &Config, req: &Request) -> Result<Vec<u8>, u16> {
    if req.method() != &Method::Post {
        return Err(405);
    }
    boardcraft_account::revoke(config, &access::bearer(req)?).map_err(access::account_status)?;
    Ok(b"null".to_vec())
}

fn navigation_boards(db: &mut Client, req: &Request) -> Result<Vec<u8>, u16> {
    if req.method() != &Method::Get {
        return Err(405);
    }
    let token = access::bearer(req)?;
    let mut tx = db.transaction().map_err(access::navigation_unavailable)?;
    let account = access::resolve(&mut tx, &token)?;
    let summaries = access::list_visible_boards(&mut tx, account)?;
    tx.commit().map_err(access::navigation_unavailable)?;
    Ok(serde_json::to_vec(&summaries).expect("serializable summaries"))
}

fn handle(
    db: &mut Client,
    scoped_board: BoardId,
    mode: Mode,
    req: &mut Request,
) -> Result<(Vec<u8>, Option<bool>), u16> {
    let path: Vec<_> = req.url().split('/').collect();
    use access::Operation::*;
    let (requested, operation, target) = match (req.method(), path.as_slice()) {
        (Method::Get, ["", "navigation", "boards", id]) if mode != Mode::Scoped => {
            (id, SelectedRead, None)
        }
        (Method::Get, ["", "boards", id]) => (id, ScopedRead, None),
        (Method::Put, ["", "boards", board, "items", id, "name"]) => {
            (board, ScopedRename, Some(id))
        }
        (Method::Put, ["", "navigation", "boards", board, "items", id, "name"])
            if mode == Mode::Preview =>
        {
            (board, SelectedRename, Some(id))
        }
        _ => return Err(404),
    };
    let requested = Uuid::parse_str(requested).map_err(|_| 404u16)?;
    let target = target
        .map(|id| Uuid::parse_str(id).ok().and_then(ItemId::new).ok_or(404u16))
        .transpose()?;
    let token = access::bearer(req)?;
    // Buffer bounded input before acquiring database locks. Do not return draft
    // validation or board data before the caller has been authorized.
    let rename = target.map(|item| (item, read_edit(req)));
    let mut tx = db.transaction().map_err(|_| {
        log::error!("begin board request failed board={requested}");
        503u16
    })?;
    let account = access::resolve(&mut tx, &token)?;
    if matches!(operation, ScopedRead | ScopedRename) && requested != scoped_board.as_uuid() {
        return Err(404);
    }
    let board = BoardId::new(requested).ok_or(404u16)?;
    let can_edit = access::authorize(&mut tx, account, board, operation)?;
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
    let hint = matches!(operation, ScopedRead)
        || (mode == Mode::Preview && matches!(operation, SelectedRead));
    Ok((body, hint.then_some(can_edit)))
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
