//! One command per process: port board [item revision draft]. Read the
//! fixture session from the private stdin pipe, never the command line.
use boardcraft_mobile_edit_client::{EditClient, EditError, NameEdit};
use std::{
    env,
    io::{self, Read},
};

fn main() -> Result<(), &'static str> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let [port, board, operation @ ..] = args.as_slice() else {
        return Err("expected port and board UUID");
    };
    let mut encoded = [0; 43];
    io::stdin()
        .lock()
        .read_exact(&mut encoded)
        .map_err(|_| "session required")?;
    let client = EditClient::with_bearer(
        port.parse().map_err(|_| "invalid port")?,
        std::str::from_utf8(&encoded).map_err(|_| "invalid session")?,
    )
    .map_err(|_| "invalid session")?;
    let board = board.parse().map_err(|_| "invalid board UUID")?;
    match operation {
        [] => {
            let rows = client.read(board).map_err(|_| "board read failed")?;
            serde_json::to_writer(io::stdout().lock(), &rows)
        }
        [item, revision, draft] => {
            let edit = NameEdit {
                revision: revision.parse().map_err(|_| "invalid revision")?,
                title: draft.clone(),
            };
            let reply = client.rename(board, item.parse().map_err(|_| "invalid item UUID")?, &edit);
            if !matches!(reply, Ok(_) | Err(EditError::InvalidDraft)) {
                return Err("unexpected rename failure");
            }
            serde_json::to_writer(io::stdout().lock(), &reply)
        }
        _ => return Err("expected port board [item revision draft]"),
    }
    .map_err(|_| "writing result failed")
}
