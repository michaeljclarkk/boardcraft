//! Single-board host for a disposable database. No database configuration crosses HTTP.
mod access;
mod browser_session;
mod preview;
use std::{io::Read, sync::Arc, thread};

use boardcraft_account::Token;
use boardcraft_board::{
    BoardId, ItemId, RenameError,
    store::{self, SaveError, StoredItem},
};
use boardcraft_mobile_edit_client::{BOARD_ACCESS_HEADER, Credentials, NameEdit, SavedItem};
use postgres::{Client, Config, NoTls};
use tiny_http::{Header, Method, Request, Response, Server};
use uuid::Uuid;
struct ReadMetadata {
    can_edit: bool,
    owner: bool,
    revision: Option<(i64, i64)>,
}
type ApiResult = Result<(Vec<u8>, Option<ReadMetadata>), u16>;

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
            let mut sessions = browser_session::Sessions::new(port);
            while let Ok(mut req) = listener.recv() {
                let wrong_host = mode == Mode::Preview && !sessions.authority_matches(&req);
                if mode == Mode::Preview
                    && !wrong_host
                    && let Some(response) = preview::asset(&req)
                {
                    let _ = req.respond(response);
                    continue;
                }
                let path = req.url().to_owned();
                let mut cookie = None;
                let result = if wrong_host {
                    Err(403)
                } else if let Some(route) = path.strip_prefix(browser_session::PREFIX) {
                    if mode != Mode::Preview {
                        Err(404)
                    } else {
                        sessions.dispatch(&mut req, route, &config, &mut db, board, &mut cookie)
                    }
                } else if req.headers().iter().any(|h| {
                    h.field.equiv("Boardcraft-Preview")
                        || h.field.equiv("Origin")
                        || h.field.equiv("Cookie")
                        || (h.field.equiv("Sec-Fetch-Mode") && path == "/session")
                }) {
                    Err(403)
                } else {
                    match path.as_str() {
                        "/session" => session(&config, &mut db, &mut req).map(|body| (body, None)),
                        "/session/logout" => logout(&config, &req).map(|body| (body, None)),
                        "/navigation/boards" if navigation => {
                            navigation_boards(&mut db, &req, access::bearer(&req))
                                .map(|body| (body, None))
                        }
                        _ => {
                            let token = access::bearer(&req);
                            handle(
                                &mut db,
                                board,
                                mode,
                                &mut req,
                                &path,
                                token,
                                ReadShape::Flat,
                            )
                        }
                    }
                };
                let mut response = match result {
                    Ok((body, hint)) => {
                        let mut response = Response::from_data(body).with_header(
                            Header::from_bytes("Content-Type", "application/json").unwrap(),
                        );
                        if let Some(ReadMetadata {
                            can_edit,
                            revision,
                            owner,
                        }) = hint
                        {
                            if let Some((revision, view_revision)) = revision {
                                response.add_header(
                                    Header::from_bytes(
                                        "Boardcraft-View-Revision",
                                        view_revision.to_string(),
                                    )
                                    .unwrap(),
                                );
                                response.add_header(
                                    Header::from_bytes(
                                        "Boardcraft-Root-Lifecycle",
                                        if owner { "owner" } else { "read" },
                                    )
                                    .unwrap(),
                                );
                                response.add_header(
                                    Header::from_bytes(
                                        "Boardcraft-Board-Revision",
                                        revision.to_string(),
                                    )
                                    .unwrap(),
                                );
                            }
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
                if let Some(header) = cookie {
                    response.add_header(header);
                }
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
            if browser_session::one(req, "Authorization")?.is_some() {
                return Err(403);
            }
            let token = login(config, req)?;
            Ok(serde_json::to_vec(token.encoded()).expect("serializable token"))
        }
        Method::Get => resolve_session(db, &access::bearer(req)?),
        _ => Err(405),
    }
}

fn login(config: &Config, req: &mut Request) -> Result<Token, u16> {
    if browser_session::one(req, "Content-Type")?.and_then(|v| v.split(';').next())
        != Some("application/json")
    {
        return Err(415);
    }
    let credentials: Credentials =
        serde_json::from_slice(&read_body(req, 64 * 1024)?).map_err(|_| 400u16)?;
    boardcraft_account::sign_in(config, &credentials.identifier, &credentials.password)
        .map_err(access::account_status)
}

fn resolve_session(db: &mut Client, token: &Token) -> Result<Vec<u8>, u16> {
    boardcraft_account::resolve_on(db, token).map_err(access::account_status)?;
    Ok(b"null".to_vec())
}

fn logout(config: &Config, req: &Request) -> Result<Vec<u8>, u16> {
    if req.method() != &Method::Post {
        return Err(405);
    }
    boardcraft_account::revoke(config, &access::bearer(req)?).map_err(access::account_status)?;
    Ok(b"null".to_vec())
}

fn navigation_boards(
    db: &mut Client,
    req: &Request,
    token: Result<Token, u16>,
) -> Result<Vec<u8>, u16> {
    if req.method() != &Method::Get {
        return Err(405);
    }
    let token = token?;
    let mut tx = db.transaction().map_err(access::navigation_unavailable)?;
    let account = access::resolve(&mut tx, &token)?;
    let summaries = access::list_visible_boards(&mut tx, account)?;
    tx.commit().map_err(access::navigation_unavailable)?;
    Ok(serde_json::to_vec(&summaries).expect("serializable summaries"))
}

enum ReadShape {
    Flat,
    Grouped,
}

fn grouped_body(
    tx: &mut postgres::Transaction<'_>,
    board: BoardId,
) -> Result<((i64, i64), Vec<u8>), u16> {
    let (revision, view_revision, groups) =
        store::load_main_table(tx, board).map_err(|err| match err {
            store::GroupReadError::Missing => 404u16,
            store::GroupReadError::TooLarge => 413,
            store::GroupReadError::Database(error) => {
                log::error!(
                    "load_groups board={} sqlstate={:?}",
                    board.as_uuid(),
                    error.code()
                );
                500
            }
        })?;
    let groups: Vec<_> = groups.into_iter().map(|group| serde_json::json!({
        "id": group.id, "name": group.name, "colour": group.colour, "revision":group.revision, "collapsed":group.collapsed, "items": group.items.into_iter().map(wire).collect::<Vec<_>>()
    })).collect();
    Ok((
        (revision, view_revision),
        serde_json::to_vec(&groups).expect("serializable groups"),
    ))
}

fn handle(
    db: &mut Client,
    scoped_board: BoardId,
    mode: Mode,
    req: &mut Request,
    path: &str,
    token: Result<Token, u16>,
    shape: ReadShape,
) -> ApiResult {
    let mut path: Vec<_> = path.split('/').collect();
    // Native Archive is opt-in, like the grouped table. Normalize only these
    // lifecycle routes so authorization, queries and commands have one owner.
    let native_archive = mode != Mode::Scoped
        && matches!(
            (req.method(), path.as_slice()),
            (
                Method::Get,
                ["", "navigation", "boards", _, "main-table", "archive"]
            ) | (
                Method::Get,
                [
                    "",
                    "navigation",
                    "boards",
                    _,
                    "main-table",
                    "archive",
                    "groups",
                    _,
                    _
                ]
            ) | (
                Method::Post,
                [
                    "",
                    "navigation",
                    "boards",
                    _,
                    "main-table",
                    "items" | "groups",
                    _,
                    "archive" | "restore"
                ]
            )
        );
    let native_group_fields = mode != Mode::Scoped
        && matches!(
            (req.method(), path.as_slice()),
            (
                Method::Post,
                ["", "navigation", "boards", _, "main-table", "groups"]
            ) | (
                Method::Put,
                ["", "navigation", "boards", _, "main-table", "groups", _]
            )
        );
    if native_archive || native_group_fields {
        path.remove(4);
    }
    // Opt-in native contract; existing selected and direct reads stay flat.
    let shape = if matches!(
        path.as_slice(),
        ["", "navigation", "boards", _, "main-table"]
    ) && req.method() == &Method::Get
    {
        ReadShape::Grouped
    } else {
        shape
    };
    use access::Operation::*;
    let (requested, operation, target) = match (req.method(), path.as_slice()) {
        (Method::Get, ["", "navigation", "boards", id]) if mode != Mode::Scoped => {
            (id, SelectedRead, None)
        }
        (Method::Get, ["", "navigation", "boards", id, "main-table"]) if mode != Mode::Scoped => {
            (id, SelectedRead, None)
        }
        (Method::Post, ["", "navigation", "boards", board, "main-table-fold"])
            if mode != Mode::Scoped =>
        {
            (board, SelectedWrite, None)
        }
        (Method::Post, ["", "navigation", "boards", board, "main-table", "items"])
            if mode != Mode::Scoped =>
        {
            (board, SelectedWrite, None)
        }
        (
            Method::Post,
            [
                "",
                "navigation",
                "boards",
                board,
                "main-table",
                "items",
                "move-selection",
            ],
        ) if mode != Mode::Scoped => (board, SelectedWrite, None),
        (
            Method::Put,
            [
                "",
                "navigation",
                "boards",
                board,
                "main-table",
                "items",
                id,
                "name",
            ],
        ) if mode != Mode::Scoped => (board, SelectedWrite, Some(id)),
        (Method::Get, ["", "boards", id]) => (id, ScopedRead, None),
        (Method::Get, ["", "navigation", "boards", board, "archive"])
        | (Method::Get, ["", "navigation", "boards", board, "archive", "groups", _, _])
            if matches!(shape, ReadShape::Grouped) || native_archive =>
        {
            (board, SelectedRead, None)
        }
        (
            Method::Post,
            [
                "",
                "navigation",
                "boards",
                board,
                "groups",
                _,
                "archive" | "restore",
            ],
        )
        | (Method::Post, ["", "navigation", "boards", board, "groups" | "group-order"])
        | (Method::Put, ["", "navigation", "boards", board, "groups", _])
            if matches!(shape, ReadShape::Grouped) || native_archive || native_group_fields =>
        {
            (board, SelectedOwner, None)
        }
        (
            Method::Post,
            [
                "",
                "navigation",
                "boards",
                board,
                "items",
                id,
                "archive" | "restore",
            ],
        ) if matches!(shape, ReadShape::Grouped) || native_archive => {
            (board, SelectedOwner, Some(id))
        }
        (Method::Put, ["", "boards", board, "items", id, "name"]) => {
            (board, ScopedRename, Some(id))
        }
        (Method::Put, ["", "navigation", "boards", board, "items", id, "name"])
            if mode == Mode::Preview =>
        {
            (board, SelectedWrite, Some(id))
        }
        (Method::Post, ["", "navigation", "boards", board, "items"])
        | (Method::Post, ["", "navigation", "boards", board, "items", "move-selection"])
            if matches!(shape, ReadShape::Grouped) =>
        {
            (board, SelectedWrite, None)
        }
        (Method::Put, ["", "navigation", "boards", board, "items", id, "group"])
            if matches!(shape, ReadShape::Grouped) =>
        {
            (board, SelectedWrite, Some(id))
        }
        (Method::Post, ["", "navigation", "boards", board, "items", id, "reorder"])
            if matches!(shape, ReadShape::Grouped) =>
        {
            (board, SelectedWrite, Some(id))
        }
        _ => return Err(404),
    };
    let requested = Uuid::parse_str(requested).map_err(|_| 404u16)?;
    let target = target
        .map(|id| Uuid::parse_str(id).ok().and_then(ItemId::new).ok_or(404u16))
        .transpose()?;
    let token = token?;
    // Buffer bounded input before acquiring database locks. Do not return draft
    // validation or board data before the caller has been authorized.
    enum Action {
        Read,
        CreateGroup(Result<CreateGroupDraft, u16>),
        GroupOrder(Result<GroupOrderDraft, u16>),
        Fold(Result<FoldDraft, u16>),
        EditGroup(String, Result<EditGroupDraft, u16>),
        ArchivePage(Option<(i64, i64)>),
        Group(String, store::Lifecycle, Result<GroupDraft, u16>),
        Lifecycle(ItemId, store::Lifecycle, Result<RootDraft, u16>),
        Rename(ItemId, Result<NameEdit, u16>),
        Create(Result<CreateDraft, u16>),
        SelectionMove(Result<SelectionMoveDraft, u16>),
        Move(ItemId, Result<MoveDraft, u16>),
        Reorder(ItemId, Result<ReorderDraft, u16>),
    }
    let action = match target {
        Some(item) if path.last() == Some(&"archive") => {
            Action::Lifecycle(item, store::Lifecycle::Archive, read_json(req))
        }
        Some(item) if path.last() == Some(&"restore") => {
            Action::Lifecycle(item, store::Lifecycle::Restore, read_json(req))
        }
        None if path.get(4) == Some(&"groups") && path.len() == 5 => {
            Action::CreateGroup(read_json(req))
        }
        None if path.get(4) == Some(&"group-order") => {
            Action::GroupOrder(read_json_limit(req, 1024 * 1024))
        }
        None if path.get(4) == Some(&"main-table-fold") => {
            Action::Fold(read_json_limit(req, 1024 * 1024))
        }
        None if path.get(4) == Some(&"groups") => {
            let id = percent_encoding::percent_decode_str(path[5])
                .decode_utf8()
                .map_err(|_| 400u16)?
                .into_owned();
            if path.len() == 6 {
                Action::EditGroup(id, read_json(req))
            } else {
                Action::Group(
                    id,
                    if path[6] == "archive" {
                        store::Lifecycle::Archive
                    } else {
                        store::Lifecycle::Restore
                    },
                    read_json(req),
                )
            }
        }
        None if path.get(4) == Some(&"archive") => Action::ArchivePage(if path.len() == 8 {
            Some((
                path[6].parse().map_err(|_| 400u16)?,
                path[7].parse().map_err(|_| 400u16)?,
            ))
        } else {
            None
        }),
        Some(item) if path.last() == Some(&"reorder") => Action::Reorder(item, read_json(req)),
        Some(item) if path.last() == Some(&"group") => Action::Move(item, read_json(req)),
        Some(item) => Action::Rename(item, read_edit(req)),
        None if path.last() == Some(&"move-selection") => {
            Action::SelectionMove(read_json_limit(req, 1024 * 1024))
        }
        None if req.method() == &Method::Post => Action::Create(read_json(req)),
        None => Action::Read,
    };
    let mut tx = db.transaction().map_err(|_| {
        log::error!("begin board request failed board={requested}");
        503u16
    })?;
    let account = access::resolve(&mut tx, &token)?;
    if matches!(operation, ScopedRead | ScopedRename) && requested != scoped_board.as_uuid() {
        return Err(404);
    }
    let board = BoardId::new(requested).ok_or(404u16)?;
    let can_edit = access::authorize(&mut tx, &account, board, operation)?;
    let owner = account.role == boardcraft_account::Role::Owner;
    let mut revision = None;
    let fold = matches!(action, Action::Fold(_));
    let body = match action {
        Action::Fold(draft) => {
            let d = draft?;
            let saved = store::set_main_table_fold(
                &mut tx,
                board,
                d.view_revision,
                &d.group_ids,
                d.collapsed,
            )
            .map_err(|e| order_error(e, requested, "set_main_table_fold"))?;
            serde_json::to_vec(&serde_json::json!({"board_id":requested,"view_revision":saved}))
                .unwrap()
        }
        Action::GroupOrder(draft) => {
            let d = draft?;
            store::reorder_groups(&mut tx, board, d.board_revision, &d.group_ids)
                .map_err(|e| order_error(e, requested, "reorder_groups"))?;
            serde_json::to_vec(
                &serde_json::json!({"board_id":requested,"board_revision":d.board_revision+1}),
            )
            .unwrap()
        }
        Action::EditGroup(id, draft) => {
            let d = draft?;
            store::edit_group(
                &mut tx,
                board,
                &id,
                d.board_revision,
                d.group_revision,
                &d.name,
                &d.colour,
            )
            .map_err(|e| order_error(e, requested, "edit_group"))?;
            serde_json::to_vec(&serde_json::json!({"board_id":requested,"group_id":id})).unwrap()
        }
        Action::CreateGroup(draft) => {
            let d = draft?;
            store::create_group(&mut tx, board, &d.id, d.board_revision, &d.name, &d.colour)
                .map_err(|e| order_error(e, requested, "create_group"))?;
            serde_json::to_vec(&serde_json::json!({"board_id":requested,"group_id":d.id})).unwrap()
        }
        Action::Group(group, action, draft) => {
            let baseline = draft?.board_revision;
            store::transition_group(&mut tx, board, &group, action, baseline)
                .map_err(|e| order_error(e, requested, "transition_group"))?;
            serde_json::to_vec(&serde_json::json!({"board_id":requested,"group_id":group,"board_revision":baseline+1})).unwrap()
        }
        Action::ArchivePage(cursor) => {
            let gp = store::archived_groups(&mut tx, board, cursor)
                .map_err(|e| order_error(e, requested, "archived_groups"))?;
            let groups:Vec<_>=gp.groups.into_iter().map(|g|serde_json::json!({"id":g.id,"name":g.name,"position":g.position,"revision":g.revision})).collect();
            let page = store::archive_page(&mut tx, board)
                .map_err(|e| order_error(e, requested, "archive_page"))?;
            let items:Vec<_>=page.items.into_iter().map(|r|serde_json::json!({"id":r.id,"title":r.title,"group_id":r.group_id,"group_name":r.group_name,"revision":r.revision,"state":r.state,"effective_state":1,"restorable":owner && r.group_active,"group_active":r.group_active})).collect();
            serde_json::to_vec(&serde_json::json!({"board_revision":page.revision,"can_manage":owner,"total":page.total,"has_more":page.total>items.len() as i64,"items":items,"groups":groups,"next_group_offset":gp.next_offset})).unwrap()
        }
        Action::Lifecycle(item, action, draft) => {
            let d = draft?;
            store::transition_root(
                &mut tx,
                account.id,
                board,
                item,
                &store::RootCommand {
                    action,
                    operation: d.operation_id,
                    board_revision: d.board_revision,
                    item_revision: d.item_revision,
                },
            )
            .map_err(|e| order_error(e, requested, "transition_root"))?;
            serde_json::to_vec(&serde_json::json!({"operation_id":d.operation_id,"item_id":item.as_uuid(),"board_id":requested})).unwrap()
        }
        Action::SelectionMove(draft) => {
            let d = draft?;
            let items: Vec<_> = d.items.into_iter().map(|r| (r.id, r.revision)).collect();
            let affected = store::move_selected_items(
                &mut tx,
                account.id,
                board,
                &store::SelectionMove {
                    operation: d.operation_id,
                    group: &d.group_id,
                    board_revision: d.board_revision,
                    items: &items,
                },
            )
            .map_err(|err| order_error(err, requested, "move_selected_items"))?;
            serde_json::to_vec(&serde_json::json!({"board_id":requested,"operation_id":d.operation_id,"affected_ids":affected})).unwrap()
        }
        Action::Reorder(item, draft) => {
            let draft = draft?;
            let command = store::Reorder {
                operation: draft.operation_id,
                group: &draft.group_id,
                anchor: draft.anchor_id,
                after: matches!(draft.side, Side::After),
                board_revision: draft.board_revision,
                item_revision: draft.item_revision,
            };
            store::reorder_group_item(&mut tx, account.id, board, item, &command)
                .map_err(|err| order_error(err, requested, "reorder_group_item"))?;
            serde_json::to_vec(&serde_json::json!({"operation_id":draft.operation_id,"board_id":requested,"item_id":item.as_uuid()})).unwrap()
        }
        Action::Create(draft) => {
            let draft = draft?;
            let item = ItemId::new(draft.id).ok_or(422u16)?;
            let saved = store::create_group_item(
                &mut tx,
                board,
                &draft.group_id,
                item,
                account.id,
                &draft.title,
            )
            .map_err(|error| match error {
                store::CreateError::InvalidGroup | store::CreateError::InvalidTitle => 422u16,
                store::CreateError::Collision => 409,
                store::CreateError::Database(e) => {
                    log::error!(
                        "create_group_item board={requested} item={} sqlstate={:?}",
                        draft.id,
                        e.code()
                    );
                    500
                }
            })?;
            group_item_body(saved)
        }
        Action::Move(item, draft) => {
            let draft = draft?;
            let saved = store::move_group_item(
                &mut tx,
                board,
                item,
                &draft.group_id,
                draft.expected_revision,
            )
            .map_err(|err| match err {
                store::MoveError::InvalidGroup => 422u16,
                store::MoveError::MissingItem => 404,
                store::MoveError::Conflict => 409,
                store::MoveError::Database(err) => {
                    log::error!(
                        "move_group_item board={requested} item={} sqlstate={:?}",
                        item.as_uuid(),
                        err.code()
                    );
                    500
                }
            })?;
            group_item_body(saved)
        }
        Action::Read if matches!((operation, shape), (SelectedRead, ReadShape::Grouped)) => {
            let (version, body) = grouped_body(&mut tx, board)?;
            revision = Some(version);
            body
        }
        Action::Read => {
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
        Action::Rename(item, edit) => {
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
    tx.commit().map_err(|error| {
        if fold {
            log::error!(
                "set_main_table_fold commit board={requested} sqlstate={:?}",
                error.code()
            );
            // A deferred constraint or trigger can reject COMMIT. Losing the
            // connection instead gives us no reliable acknowledgment either way.
            return if error.as_db_error().is_some_and(|e| {
                e.severity() == "ERROR"
                    && !e.code().code().starts_with("08")
                    && e.code().code() != "40003"
            }) {
                500u16
            } else {
                503u16
            };
        }
        log::error!("board request commit failed board={requested}");
        500u16
    })?;
    let hint = matches!(operation, ScopedRead)
        || ((mode == Mode::Preview || revision.is_some()) && matches!(operation, SelectedRead));
    Ok((
        body,
        hint.then_some(ReadMetadata {
            can_edit,
            revision,
            owner,
        }),
    ))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Side {
    Before,
    After,
}

fn order_error(err: store::ReorderError, board: Uuid, op: &str) -> u16 {
    match err {
        store::ReorderError::Conflict => 409,
        store::ReorderError::Invalid => 422,
        store::ReorderError::Missing => 404,
        store::ReorderError::TooLarge => 413,
        store::ReorderError::Database(e) => {
            log::error!("{op} board={board} sqlstate={:?}", e.code());
            500
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionMoveDraft {
    operation_id: Uuid,
    group_id: String,
    board_revision: i64,
    items: Vec<SelectedRoot>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RootDraft {
    operation_id: Uuid,
    board_revision: i64,
    item_revision: i64,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupDraft {
    board_revision: i64,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupOrderDraft {
    board_revision: i64,
    group_ids: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FoldDraft {
    view_revision: i64,
    group_ids: Vec<String>,
    collapsed: bool,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EditGroupDraft {
    board_revision: i64,
    group_revision: i64,
    name: String,
    colour: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateGroupDraft {
    id: String,
    board_revision: i64,
    name: String,
    colour: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectedRoot {
    id: Uuid,
    revision: i64,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReorderDraft {
    operation_id: Uuid,
    group_id: String,
    anchor_id: Uuid,
    side: Side,
    board_revision: i64,
    item_revision: i64,
}

fn read_edit(req: &mut Request) -> Result<NameEdit, u16> {
    serde_json::from_slice(&read_body(req, 64 * 1024)?).map_err(|_| 400)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateDraft {
    id: Uuid,
    group_id: String,
    title: String,
}

fn group_item_body(saved: store::GroupItem) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"id":saved.item.id.as_uuid(),"group_id":saved.group_id,"title":saved.item.title,"revision":saved.item.revision})).unwrap()
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveDraft {
    group_id: String,
    expected_revision: i64,
}

fn read_json<T: serde::de::DeserializeOwned>(req: &mut Request) -> Result<T, u16> {
    read_json_limit(req, 64 * 1024)
}

fn read_json_limit<T: serde::de::DeserializeOwned>(
    req: &mut Request,
    limit: u64,
) -> Result<T, u16> {
    if browser_session::one(req, "Content-Type")?.and_then(|v| v.split(';').next())
        != Some("application/json")
    {
        return Err(415);
    }
    serde_json::from_slice(&read_body(req, limit)?).map_err(|_| 400)
}

fn read_body(req: &mut Request, limit: u64) -> Result<Vec<u8>, u16> {
    let mut body = Vec::new();
    req.as_reader()
        .take(limit + 1)
        .read_to_end(&mut body)
        .map_err(|_| 400u16)?;
    if body.len() as u64 > limit {
        return Err(413);
    }
    Ok(body)
}
