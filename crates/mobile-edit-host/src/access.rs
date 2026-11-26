use boardcraft_account::{Account, Error, Token};
use boardcraft_board::BoardId;
use postgres::Transaction;
use tiny_http::Request;
use uuid::Uuid;

pub(super) fn bearer(req: &Request) -> Result<Token, u16> {
    let mut values = req
        .headers()
        .iter()
        .filter(|h| h.field.equiv("Authorization"));
    let (scheme, raw) = values
        .next()
        .and_then(|h| h.value.as_str().split_once(' '))
        .ok_or(401u16)?;
    if values.next().is_some() || !scheme.eq_ignore_ascii_case("Bearer") {
        return Err(401);
    }
    Token::from_encoded(raw).map_err(|_| 401)
}

pub(super) fn resolve(tx: &mut Transaction<'_>, token: &Token) -> Result<Account, u16> {
    boardcraft_account::resolve_on(tx, token).map_err(account_status)
}

pub(super) fn account_status(e: Error) -> u16 {
    match e {
        Error::Rejected => 401,
        Error::Unavailable => 503,
    }
}

pub(super) fn authorize(
    tx: &mut Transaction<'_>,
    account: Account,
    board: BoardId,
    write: bool,
) -> Result<(), u16> {
    if write && account.role.is_read_only() {
        return Err(403);
    }
    let board = board.as_uuid();
    let unavailable = |_| {
        log::error!("authorize board={board} unavailable");
        503u16
    };
    let policy = tx
        .query_opt(
            "SELECT creator_id FROM open_workspace_private_boards WHERE board_id=$1 FOR SHARE",
            &[&board],
        )
        .map_err(unavailable)?
        .ok_or(403u16)?;
    if policy.get::<_, Uuid>(0) == account.id {
        return Ok(());
    }
    tx.query_opt(
        "SELECT 1 FROM private_board_subscribers WHERE board_id=$1 AND account_id=$2 FOR SHARE",
        &[&board, &account.id],
    )
    .map_err(unavailable)?
    .ok_or(403u16)?;
    Ok(())
}
