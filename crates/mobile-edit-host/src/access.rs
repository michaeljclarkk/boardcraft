use boardcraft_account::{Account, Error, Token};
use boardcraft_board::BoardId;
use boardcraft_mobile_edit_client::BoardSummary;
use postgres::{Client, Transaction};
use tiny_http::Request;

pub(super) fn migrate(db: &mut Client) -> Result<(), postgres::Error> {
    let mut tx = db.transaction()?;
    tx.batch_execute(
        "SELECT pg_advisory_xact_lock(hashtext('boardcraft.board.access.migrations'))",
    )?;
    if !tx
        .query_one("SELECT to_regclass('board_policies') IS NOT NULL", &[])?
        .get::<_, bool>(0)
    {
        tx.batch_execute(include_str!("../migrations/0001_private_board_access.sql"))?;
        tx.batch_execute(include_str!("../migrations/0002_navigation.sql"))?;
    }
    tx.commit()
}

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

// The target operation locks each row supplying access through commit. Listing
// needs only a statement snapshot and never acquires a durable grant.
fn workspace_visible(lock: &str) -> String {
    format!(
        "w.active AND (w.is_open OR w.creator_id=$1 OR EXISTS (
        SELECT 1 FROM workspace_members WHERE workspace_id=w.id AND account_id=$1 {lock}))"
    )
}

fn board_visible(lock: &str) -> String {
    format!(
        "b.active AND (b.kind='main' OR b.creator_id=$1 OR EXISTS (
        SELECT 1 FROM board_subscribers WHERE board_id=b.board_id AND account_id=$1 {lock}))"
    )
}

pub(super) fn navigation_unavailable(_: postgres::Error) -> u16 {
    log::error!("list_visible_boards unavailable");
    503
}

pub(super) fn list_visible_boards(
    tx: &mut Transaction<'_>,
    account: Account,
) -> Result<Vec<BoardSummary>, u16> {
    let sql = format!(
        "WITH visible_workspaces AS (
        SELECT w.id FROM workspaces w WHERE {} ORDER BY w.position,w.name,w.id LIMIT 2000
    ) SELECT b.board_id,b.workspace_id,b.name FROM board_policies b
        JOIN visible_workspaces w ON w.id=b.workspace_id WHERE {}
        ORDER BY b.position,b.name,b.board_id LIMIT 2000",
        workspace_visible(""),
        board_visible("")
    );
    Ok(tx
        .query(&sql, &[&account.id])
        .map_err(navigation_unavailable)?
        .into_iter()
        .map(|row| BoardSummary {
            id: row.get(0),
            workspace_id: row.get(1),
            name: row.get(2),
        })
        .collect())
}

#[derive(Clone, Copy)]
pub(super) enum Operation {
    ScopedRead,
    ScopedRename,
    SelectedRead,
    SelectedWrite,
    SelectedLifecycle,
}

pub(super) fn authorize(
    tx: &mut Transaction<'_>,
    account: &Account,
    board: BoardId,
    operation: Operation,
) -> Result<bool, u16> {
    let can_edit = !account.role.is_read_only();
    if matches!(operation, Operation::SelectedLifecycle)
        && account.role != boardcraft_account::Role::Owner
    {
        return Err(403);
    }
    if matches!(
        operation,
        Operation::ScopedRename | Operation::SelectedWrite
    ) && !can_edit
    {
        return Err(403);
    }
    let board = board.as_uuid();
    let unavailable = |_| {
        log::error!("authorize board={board} unavailable");
        503u16
    };
    let allow_legacy = matches!(operation, Operation::ScopedRead | Operation::ScopedRename);
    let sql = format!(
        "SELECT 1 FROM board_policies b WHERE b.board_id=$2 AND {}
        AND (($3 AND b.workspace_id IS NULL) OR EXISTS (SELECT 1 FROM workspaces w
             WHERE w.id=b.workspace_id AND {} FOR SHARE)) FOR SHARE",
        board_visible("FOR SHARE"),
        workspace_visible("FOR SHARE")
    );
    tx.query_opt(&sql, &[&account.id, &board, &allow_legacy])
        .map_err(unavailable)?
        .ok_or(403u16)?;
    Ok(can_edit)
}
