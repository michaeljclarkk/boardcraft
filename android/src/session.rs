//! Platform storage boundary. Only pass validated bearers across JNI, never credentials.
use super::{Connection, session_store};
use boardcraft_mobile_edit_client::{EditClient, EditError, SavedItem};
use gpui_mobile::android::jni::{activity, with_env};

pub enum SessionError {
    Local,
    Remote(EditError),
}

impl From<EditError> for SessionError {
    fn from(error: EditError) -> Self {
        Self::Remote(error)
    }
}

enum Operation<'a> {
    Load,
    Save(&'a str),
    Forget,
}

fn storage(port: u16, op: Operation) -> Result<Option<String>, SessionError> {
    let action = match op {
        Operation::Load => "load",
        Operation::Save(_) => "save",
        Operation::Forget => "forget",
    };
    with_env(|env| {
        let app = activity(env)?;
        let result = match op {
            Operation::Load => session_store::load(env, &app, port),
            Operation::Save(encoded) => {
                session_store::save(env, &app, port, encoded).map(|()| None)
            }
            Operation::Forget => session_store::forget(env, &app, port).map(|()| None),
        };
        result.map_err(|_| {
            env.exception_clear();
            "Session storage unavailable".to_owned()
        })
    })
    .map_err(|_| {
        eprintln!("Session store {action} failed for loopback port {port}");
        SessionError::Local
    })
}

fn load(port: u16) -> Result<Option<EditClient>, SessionError> {
    storage(port, Operation::Load)?
        .map(|s| EditClient::with_bearer(port, &s).map_err(|_| SessionError::Local))
        .transpose()
}

pub fn save(port: u16, encoded: &str) -> Result<(), SessionError> {
    storage(port, Operation::Save(encoded)).map(drop)
}

pub fn forget(port: u16) -> Result<(), SessionError> {
    storage(port, Operation::Forget).map(drop)
}

pub enum Open {
    Missing,
    Unreadable,
    Rejected,
    Unavailable,
    Denied(EditClient),
    Board(EditClient, Vec<SavedItem>),
    Browse(EditClient, Vec<boardcraft_mobile_edit_client::BoardSummary>),
}

/// Called only on a worker. Issuance, cold launch and explicit retry all pass
/// through fresh account resolution, followed by a separately authorized read.
pub fn open(conn: Connection, issued: Option<EditClient>) -> Open {
    let client = match issued {
        Some(c) => c,
        None => match load(conn.port) {
            Ok(Some(c)) => c,
            Ok(None) => return Open::Missing,
            Err(_) => return Open::Unreadable,
        },
    };
    match client.resolve() {
        Ok(()) => (),
        Err(EditError::SessionRejected) => return Open::Rejected,
        Err(_) => return Open::Unavailable,
    }
    let Some(board) = conn.board else {
        return match client.list_boards() {
            Ok(list) => Open::Browse(client, list),
            Err(EditError::SessionRejected) => Open::Rejected,
            Err(_) => Open::Unavailable,
        };
    };
    match client.read(board) {
        Ok(rows) => Open::Board(client, rows),
        Err(EditError::PermissionDenied) => Open::Denied(client),
        Err(EditError::SessionRejected) => Open::Rejected,
        Err(_) => Open::Unavailable,
    }
}
