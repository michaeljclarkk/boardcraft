//! Single-board host for a disposable database. No database configuration crosses HTTP.
use std::{io::Read, sync::Arc, thread};

use boardcraft_board::{
    BoardId, ItemId, RenameError,
    store::{self, SaveError, StoredItem},
};
use boardcraft_mobile_edit_client::{NameEdit, SavedItem};
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
        let server = Arc::new(
            Server::http((std::net::Ipv4Addr::LOCALHOST, 0)).map_err(|_| "loopback bind failed")?,
        );
        let port = server.server_addr().to_ip().expect("TCP listener").port();
        let listener = server.clone();
        let worker = thread::spawn(move || {
            while let Ok(mut req) = listener.recv() {
                let response = match handle(&mut db, board, &mut req) {
                    Ok(body) => Response::from_data(body).with_header(
                        Header::from_bytes("Content-Type", "application/json").unwrap(),
                    ),
                    Err(status) => Response::from_data(Vec::new()).with_status_code(status),
                };
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

fn handle(db: &mut Client, board: BoardId, req: &mut Request) -> Result<Vec<u8>, u16> {
    let path: Vec<_> = req.url().split('/').collect();
    let ["", "boards", requested, route @ ..] = path.as_slice() else {
        return Err(404);
    };
    if Uuid::parse_str(requested).ok() != Some(board.as_uuid()) {
        return Err(404);
    }
    match (req.method(), route) {
        (Method::Get, []) => {
            let rows = store::load_items(db, board)
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
            Ok(serde_json::to_vec(&items).expect("serializable item list"))
        }
        (Method::Put, ["items", id, "name"]) => {
            let item = Uuid::parse_str(id)
                .ok()
                .and_then(ItemId::new)
                .ok_or(404u16)?;
            const MAX_BODY: u64 = 64 * 1024;
            let mut body = Vec::new();
            req.as_reader()
                .take(MAX_BODY + 1)
                .read_to_end(&mut body)
                .map_err(|_| 400u16)?;
            if body.len() as u64 > MAX_BODY {
                return Err(413);
            }
            let edit: NameEdit = serde_json::from_slice(&body).map_err(|_| 400u16)?;
            let saved = store::rename_item_at_revision(db, board, item, edit.revision, &edit.title)
                .map_err(|error| match error {
                    SaveError::Rejected(RenameError::MissingItem(_)) => 404u16,
                    SaveError::Rejected(_) => 422,
                    SaveError::Conflict { .. } => 409,
                    SaveError::Database(_) => 500,
                })?;
            Ok(serde_json::to_vec(&wire(saved)).expect("serializable item"))
        }
        _ => Err(404),
    }
}
