//! One command per process: port board [item revision draft].
use boardcraft_mobile_edit_client::{EditClient, EditError, NameEdit};
use std::{env, io};

fn main() -> Result<(), &'static str> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let [port, board, operation @ ..] = args.as_slice() else {
        return Err("expected port and board UUID");
    };
    let client = EditClient::new(port.parse().map_err(|_| "invalid port")?);
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
