#[path = "fixture/browser_transport.rs"]
mod browser_transport;
#[path = "fixture/caller.rs"]
mod caller;
#[path = "fixture/create.rs"]
mod create;
mod fixture;
#[path = "fixture/navigation.rs"]
mod navigation;
#[path = "fixture/peer.rs"]
mod peer;
#[path = "fixture/selection.rs"]
mod selection;
#[path = "fixture/stored_session.rs"]
mod stored_session;

#[test]
fn preview_create_requires_current_group_access() {
    create::access();
}

#[test]
fn preview_create_is_atomic_and_replay_safe() {
    create::atomic();
}

#[test]
fn preview_browser_session_is_origin_bound() {
    browser_transport::check();
}

#[test]
fn preview_groups_require_active_membership() {
    browser_transport::groups();
}

#[test]
fn group_migration_preserves_flat_boards() {
    use boardcraft_board::{Board, BoardId, ItemId, store};
    use uuid::Uuid;
    let schema = fixture::Schema::new();
    let connect = || schema.config.connect(postgres::NoTls).unwrap();
    let empty = selection::SIBLING;
    let full = selection::SELECTED;
    let mut db = connect();
    db.batch_execute(include_str!("../../board/migrations/0001_boards.sql"))
        .unwrap();
    db.batch_execute(include_str!(
        "../../board/migrations/0002_item_revision.sql"
    ))
    .unwrap();
    db.batch_execute("CREATE TABLE rustday_migrations(version integer PRIMARY KEY); INSERT INTO rustday_migrations VALUES (1),(2)").unwrap();
    db.execute("INSERT INTO boards VALUES ($1),($2)", &[&empty, &full])
        .unwrap();
    db.execute("INSERT INTO board_items(id,board_id,position,title,revision) VALUES ($1,$3,9,' Padded ',5),($2,$3,3,'Review',1)", &[&SECOND,&FIRST,&full]).unwrap();
    for _ in 0..2 {
        store::migrate(&mut db).unwrap();
        db = connect();
        let groups = db
            .query(
                "SELECT board_id,id,name,position,active FROM board_groups ORDER BY board_id",
                &[],
            )
            .unwrap();
        assert_eq!(groups.len(), 2);
        for (r, board) in groups.iter().zip([full, empty]) {
            assert_eq!(
                (
                    r.get::<_, Uuid>(0),
                    r.get::<_, &str>(1),
                    r.get::<_, &str>(2),
                    r.get::<_, i64>(3),
                    r.get::<_, bool>(4)
                ),
                (board, "main", "Items", 0, true)
            );
        }
        let items = db.query("SELECT board_id,group_id,id,position,title,revision FROM board_items ORDER BY position",&[]).unwrap();
        assert_eq!(items.len(), 2);
        for (r, (id, pos, title, rev)) in items
            .iter()
            .zip([(FIRST, 3, "Review", 1), (SECOND, 9, " Padded ", 5)])
        {
            assert_eq!(
                (
                    r.get::<_, Uuid>(0),
                    r.get::<_, &str>(1),
                    r.get::<_, Uuid>(2),
                    r.get::<_, i64>(3),
                    r.get::<_, &str>(4),
                    r.get::<_, i64>(5)
                ),
                (full, "main", id, pos, title, rev)
            );
        }
        assert!(
            store::load_items(&mut db, BoardId::new(empty).unwrap())
                .unwrap()
                .unwrap()
                .is_empty()
        );
    }
    let mut snapshot = Board::new(BoardId::new(Uuid::new_v4()).unwrap());
    snapshot
        .add_item(ItemId::new(Uuid::from_u128(91)).unwrap(), "New")
        .unwrap();
    store::create_board(&mut db, &snapshot).unwrap();
    let groups = store::load_groups(&mut connect(), snapshot.id())
        .ok()
        .unwrap();
    assert_eq!(groups.len(), 1);
    let g = &groups[0];
    assert_eq!(g.id, "main");
    assert_eq!(g.items.len(), 1);
    assert_eq!(
        (
            g.items[0].id.as_uuid().as_u128(),
            g.items[0].title.as_str(),
            g.items[0].revision
        ),
        (91, "New", 1)
    );
    db.execute("INSERT INTO board_groups SELECT $1,'empty-'||n,'Empty',0,true FROM generate_series(1,$2::bigint) n", &[&empty,&(store::GROUPED_ROW_LIMIT-1)]).unwrap();
    let err = db
        .execute(
            "UPDATE board_items SET group_id='empty-1' WHERE id=$1",
            &[&FIRST],
        )
        .unwrap_err();
    assert_eq!(
        err.code(),
        Some(&postgres::error::SqlState::FOREIGN_KEY_VIOLATION)
    );
    let empty = BoardId::new(empty).unwrap();
    assert_eq!(
        store::load_groups(&mut db, empty).ok().unwrap().len(),
        store::GROUPED_ROW_LIMIT as usize
    );
    db.execute(
        "INSERT INTO board_groups VALUES ($1,'overflow','Overflow',0,true)",
        &[&empty.as_uuid()],
    )
    .unwrap();
    assert!(matches!(
        store::load_groups(&mut db, empty),
        Err(store::GroupReadError::TooLarge)
    ));
}

use boardcraft_account::{Role, revoke};
use boardcraft_mobile_edit_client::{Credentials, EditClient, EditError, NameEdit, SavedItem};
use caller::Caller;
use fixture::{FIRST, Fixture, SECOND};

fn rows(second: &str, revision: i64) -> [SavedItem; 2] {
    [(FIRST, "Review", 1), (SECOND, second, revision)].map(|(id, title, revision)| SavedItem {
        id,
        title: title.into(),
        revision,
    })
}

fn rename(client: &EditClient, f: &Fixture, revision: i64) -> Result<SavedItem, EditError> {
    client.rename(
        f.board,
        SECOND,
        &NameEdit {
            title: " Coral ".into(),
            revision,
        },
    )
}

#[test]
fn list_visible_boards_in_navigation_order() {
    let f = Fixture::start();
    let catalog = navigation::Catalog::seed(&f);
    let host = catalog.host(&f);
    let member = catalog.member.client(host.port);
    let viewer = catalog.viewer.client(host.port);
    assert_eq!(member.list_boards().unwrap(), catalog.open_for_member());
    assert_eq!(
        viewer.list_boards().unwrap(),
        [
            catalog.ash.clone(),
            catalog.amber.clone(),
            catalog.quartz.clone(),
            catalog.pine.clone()
        ]
    );
    assert_eq!(
        EditClient::new(host.port).list_boards(),
        Err(EditError::SessionRejected)
    );
    assert_eq!(
        catalog.member.client(f.host.port).list_boards(),
        Err(EditError::Missing)
    );
    catalog.check_state(&f, "member", false, true);
    catalog.check_limits(&f, &viewer);
}

#[test]
fn preview_document_preserves_scoped_api() {
    use boardcraft_board::BoardId;
    use boardcraft_mobile_edit_host::Host;
    use std::io::{Read, Write};

    fn request(port: u16, method: &str, path: &str, metadata: &str) -> (u16, String, String) {
        let mut stream =
            std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        write!(
            stream,
            "{method} {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n{metadata}\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        let status = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
        (status, headers.to_ascii_lowercase(), body.to_owned())
    }

    let f = Fixture::start();
    let s = selection::Selection::new(&f);
    let preview = Host::start_preview(&f.schema.config, BoardId::new(f.board).unwrap()).unwrap();
    let target = format!("/boards/{}", f.board);
    let other = format!("/boards/{}", selection::SELECTED);
    let mode = "Sec-Fetch-Mode: navigate\r\n";
    let dest = "Sec-Fetch-Dest: document\r\n";
    let doc = format!("{mode}{dest}");
    for path in [&target, &other] {
        let (status, headers, body) = request(preview.port, "GET", path, &doc);
        assert_eq!(status, 200);
        for header in [
            "content-type: text/html",
            "cache-control: no-store",
            "content-security-policy: default-src 'none'",
        ] {
            assert!(headers.contains(header));
        }
        assert_eq!(body, include_str!("../src/preview/index.html"));
    }
    for metadata in [
        "",
        mode,
        dest,
        "Sec-Fetch-Mode: navigate\r\nSec-Fetch-Dest: iframe\r\n",
    ] {
        assert_eq!(request(preview.port, "GET", &target, metadata).0, 401);
    }
    for port in [s.host.port, f.host.port] {
        assert_eq!(request(port, "GET", &target, &doc).0, 401);
    }
    for path in [
        format!("{other}/"),
        format!("{other}?view=main"),
        other.replace('-', ""),
        format!("/boards/{}", uuid::Uuid::nil()),
        format!("/boards/{:X}", selection::SELECTED.hyphenated()),
    ] {
        let (status, _, body) = request(preview.port, "GET", &path, &doc);
        assert!(status >= 400 && body.is_empty());
    }
    assert_eq!(request(preview.port, "POST", &target, &doc).0, 404);
    let client = EditClient::with_bearer(preview.port, s.token.encoded()).unwrap();
    assert_eq!(client.read(f.board).unwrap(), rows("Review", 1));
    assert_eq!(client.read(selection::SELECTED), Err(EditError::Missing));
    assert_eq!(
        client.read_selected_board(selection::SELECTED).unwrap(),
        selection::selected_rows()
    );
    s.check(&f, true);
}

#[test]
fn preview_selected_rename_requires_current_access() {
    use selection::{SELECTED, Selection};
    use std::io::{Read, Write};
    let f = Fixture::start();
    let s = Selection::new(&f);
    let preview = boardcraft_mobile_edit_host::Host::start_preview(
        &f.schema.config,
        boardcraft_board::BoardId::new(f.board).unwrap(),
    )
    .unwrap();
    let selected = |board, item| format!("/navigation/boards/{board}/items/{item}/name");
    let item = uuid::Uuid::from_u128(61);
    let path = selected(SELECTED, item);
    let denied = |port, path: &str, authenticated, expected, joined| {
        let mut stream =
            std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let body = r#"{"title":"Coral","revision":1}"#;
        write!(stream, "PUT {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n", body.len()).unwrap();
        if authenticated {
            write!(stream, "Authorization: Bearer {}\r\n", s.token.encoded()).unwrap();
        }
        write!(stream, "\r\n{body}").unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        assert_eq!(headers.split_whitespace().nth(1).unwrap(), expected);
        assert!(body.is_empty());
        s.check_rows(&f, joined);
        f.check_store(&rows("Review", 1));
    };
    denied(preview.port, &path, false, "401", true);
    for port in [f.host.port, s.host.port] {
        denied(port, &path, true, "404", true);
    }
    denied(
        preview.port,
        path.strip_prefix("/navigation").unwrap(),
        true,
        "404",
        true,
    );
    denied(preview.port, &selected(f.board, SECOND), true, "403", true);
    denied(
        preview.port,
        &selected(SELECTED, uuid::Uuid::from_u128(62)),
        true,
        "404",
        true,
    );
    s.membership(&f, false);
    denied(preview.port, &path, true, "403", false);
    s.check(&f, false);
    s.membership(&f, true);
    s.session.revoke(&f);
    denied(preview.port, &path, true, "401", true);
}

#[test]
fn selected_board_read_checks_current_membership() {
    use selection::*;
    let f = Fixture::start();
    let s = Selection::new(&f);
    let client = EditClient::with_bearer(s.host.port, s.token.encoded()).unwrap();
    let ordinary = EditClient::with_bearer(f.host.port, s.token.encoded()).unwrap();
    assert_eq!(
        ordinary.read_selected_board(SELECTED),
        Err(EditError::Missing)
    );
    for c in [&ordinary, &client] {
        assert_eq!(c.read(SELECTED), Err(EditError::Missing));
        assert_eq!(
            c.rename(
                SELECTED,
                selected_rows()[0].id,
                &NameEdit {
                    title: "New".into(),
                    revision: 1
                }
            ),
            Err(EditError::Missing)
        );
    }
    assert_eq!(
        client.read_selected_board(SELECTED).unwrap(),
        selected_rows()
    );
    assert_eq!(
        client.read_selected_board(HIDDEN),
        Err(EditError::PermissionDenied)
    );
    assert_eq!(
        client.read_selected_board(f.board),
        Err(EditError::PermissionDenied)
    );
    s.check(&f, true);
    s.membership(&f, false);
    assert_eq!(
        client.read_selected_board(SELECTED),
        Err(EditError::PermissionDenied)
    );
    // Check the denied response body too, independently of the client's error mapping.
    use std::io::{Read, Write};
    let mut stream =
        std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, s.host.port)).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    write!(stream,"GET /navigation/boards/{SELECTED} HTTP/1.0\r\nHost: localhost\r\nAuthorization: Bearer {}\r\n\r\n",s.token.encoded()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(headers.lines().next().unwrap().contains(" 403 ") && body.is_empty());
    client.resolve().unwrap();
    s.check(&f, false);
    s.session.revoke(&f);
    assert_eq!(
        client.read_selected_board(SELECTED),
        Err(EditError::SessionRejected)
    );
}

#[test]
fn workspace_membership_changes_refresh_list_and_access() {
    let f = Fixture::start();
    let catalog = navigation::Catalog::seed(&f);
    let host = catalog.host(&f);
    let client = catalog.member.client(host.port);
    let original = rows("Review", 1);
    assert_eq!(client.list_boards().unwrap(), catalog.open_for_member());
    denied(&client, &f, EditError::PermissionDenied, &original);
    catalog.check_state(&f, "member", false, true);

    let membership: &[&(dyn postgres::types::ToSql + Sync)] =
        &[&catalog.ash.workspace_id, &catalog.member.id];
    f.connect()
        .execute("INSERT INTO workspace_members VALUES ($1,$2)", membership)
        .unwrap();
    let joined = catalog.joined_for_member();
    assert_eq!(client.list_boards().unwrap(), joined);
    let read = client.read_with_access(f.board).unwrap();
    assert!(read.can_edit);
    assert_eq!(read.rows, original);
    assert_eq!(client.read(catalog.amber.id), Err(EditError::Missing));
    assert_eq!(
        client.rename(
            catalog.amber.id,
            SECOND,
            &NameEdit {
                title: "Coral".into(),
                revision: 1
            }
        ),
        Err(EditError::Missing)
    );
    catalog.check_state(&f, "member", true, true);

    f.connect()
        .execute(
            "UPDATE accounts SET role='viewer' WHERE id=$1",
            &[&catalog.member.id],
        )
        .unwrap();
    assert_eq!(client.list_boards().unwrap(), joined);
    let read = client.read_with_access(f.board).unwrap();
    assert!(!read.can_edit);
    assert_eq!(read.rows, original);
    assert_eq!(rename(&client, &f, 1), Err(EditError::PermissionDenied));
    catalog.check_state(&f, "viewer", true, true);

    f.connect()
        .execute(
            "DELETE FROM workspace_members WHERE workspace_id=$1 AND account_id=$2",
            membership,
        )
        .unwrap();
    assert_eq!(client.list_boards().unwrap(), catalog.open_for_member());
    denied(&client, &f, EditError::PermissionDenied, &original);
    catalog.check_state(&f, "viewer", false, true);

    f.connect()
        .batch_execute("ALTER TABLE board_subscribers RENAME TO unavailable_subscribers")
        .unwrap();
    assert_eq!(client.list_boards(), Err(EditError::AccessUnavailable));
    f.connect()
        .batch_execute("ALTER TABLE unavailable_subscribers RENAME TO board_subscribers")
        .unwrap();
    assert_eq!(client.list_boards().unwrap(), catalog.open_for_member());

    f.connect()
        .execute(
            "UPDATE workspaces SET is_open=false WHERE id=$1",
            &[&catalog.amber.workspace_id],
        )
        .unwrap();
    assert!(client.list_boards().unwrap().is_empty());
    client.resolve().unwrap();
    catalog.check_state(&f, "viewer", false, true);
    revoke(&f.schema.config, &catalog.member.token).unwrap();
    assert_eq!(client.list_boards(), Err(EditError::SessionRejected));
    catalog.check_state(&f, "viewer", false, false);
}

fn denied(client: &EditClient, f: &Fixture, error: EditError, expected: &[SavedItem]) {
    assert_eq!(client.read(f.board).unwrap_err(), error);
    assert_eq!(
        client
            .rename(
                f.board,
                SECOND,
                &NameEdit {
                    title: "Must not persist".into(),
                    revision: expected[1].revision
                }
            )
            .unwrap_err(),
        error
    );
    f.check_store(expected);
}

#[test]
fn stored_bearer_reopens_only_while_authorized() {
    use stored_session::{StoredSession, assert_no_session};
    let password = uuid::Uuid::new_v4().simple().to_string();
    // Upgrade a populated packet 14 schema. Its subscriber still has direct
    // private access, even though there was no workspace ID or catalog name.
    let f = Fixture::before_host(["Review", "Review"], |config, db, board| {
        boardcraft_account::migrate(config).unwrap();
        db.batch_execute(include_str!("../migrations/0001_private_board_access.sql"))
            .unwrap();
        let owner = fixture::seed_account_on(
            db,
            "creator",
            &uuid::Uuid::new_v4().to_string(),
            Role::Member,
        );
        let member = fixture::seed_account_on(db, "boardmember", &password, Role::Member);
        db.execute(
            "INSERT INTO open_workspace_private_boards VALUES ($1,$2)",
            &[&board, &owner],
        )
        .unwrap();
        db.execute(
            "INSERT INTO private_board_subscribers VALUES ($1,$2)",
            &[&board, &member],
        )
        .unwrap();
    });
    let member = f
        .connect()
        .query_one("SELECT id FROM accounts WHERE username='boardmember'", &[])
        .unwrap()
        .get(0);
    let credentials = || Credentials {
        identifier: "boardmember".into(),
        password: password.clone(),
    };
    let port = f.host.port;
    assert!(matches!(
        EditClient::sign_in_with_storage(port, credentials(), |_| Err(EditError::HostFault)),
        Err(EditError::HostFault)
    ));
    assert_no_session(&f);
    let mut stored = String::new();
    let client = EditClient::sign_in_with_storage(port, credentials(), |s| {
        stored = s.to_owned();
        Ok::<_, EditError>(())
    })
    .unwrap();
    let discovery = boardcraft_mobile_edit_host::Host::start_with_navigation(
        &f.schema.config,
        boardcraft_board::BoardId::new(f.board).unwrap(),
    )
    .unwrap();
    assert!(
        EditClient::with_bearer(discovery.port, &stored)
            .unwrap()
            .list_boards()
            .unwrap()
            .is_empty()
    );
    rename(&client, &f, 1).unwrap();
    drop(client);
    let original = StoredSession::only(&f, member);
    let reopened = EditClient::with_bearer(port, &stored).unwrap();
    reopened.resolve().unwrap();
    let saved = rows("Coral", 2);
    assert_eq!(reopened.read(f.board).unwrap(), saved);
    f.check_store(&saved);
    f.connect()
        .execute("DELETE FROM board_policies WHERE board_id=$1", &[&f.board])
        .unwrap();
    reopened.resolve().unwrap();
    denied(&reopened, &f, EditError::PermissionDenied, &saved);
    assert!(StoredSession::only(&f, member) == original);
    original.revoke(&f);
    assert_eq!(reopened.resolve(), Err(EditError::SessionRejected));
    f.check_store(&saved);
}

#[test]
fn explicit_logout_isolates_sessions() {
    let f = Fixture::start();
    let password = uuid::Uuid::new_v4().simple().to_string();
    let member = f.seed_account("boardmember", &password, Role::Member);
    f.set_creator(member);
    let credentials = || Credentials {
        identifier: "boardmember".into(),
        password: password.clone(),
    };
    let a = EditClient::sign_in(f.host.port, credentials()).unwrap();
    let b = peer::Peer::sign_in(&f, member, credentials());
    let original = rows("Review", 1);
    assert_eq!(a.read(f.board).unwrap(), original);
    assert_eq!(b.client.read(f.board).unwrap(), original);
    assert_eq!(
        EditClient::new(f.host.port).logout(),
        Err(EditError::SessionRejected)
    );
    b.check(&f, true);

    a.logout().unwrap();
    b.check(&f, false);
    f.check_store(&original);
    assert_eq!(a.resolve(), Err(EditError::SessionRejected));
    denied(&a, &f, EditError::SessionRejected, &original);
    a.logout().unwrap(); // A lost acknowledgment can be retried explicitly.
    b.check(&f, false);
    b.client.resolve().unwrap();
    assert_eq!(b.client.read(f.board).unwrap(), original);
}

#[test]
fn connected_member_sign_in_reject_then_correct() {
    let f = Fixture::with_titles(["Review sketch", "Ink"]);
    let password = uuid::Uuid::new_v4().simple().to_string();
    let id = f.seed_account("boardmember", &password, Role::Member);
    f.set_creator(id);
    let login = |password: String| {
        EditClient::sign_in(
            f.host.port,
            Credentials {
                identifier: "boardmember".into(),
                password,
            },
        )
    };
    let sessions = || {
        f.connect()
            .query("SELECT account_id FROM account_sessions", &[])
            .unwrap()
            .into_iter()
            .map(|r| r.get::<_, uuid::Uuid>(0))
            .collect::<Vec<_>>()
    };
    let mut original = rows("Ink", 1);
    original[0].title = "Review sketch".into();
    assert!(matches!(
        login(format!("{password}x")),
        Err(EditError::SessionRejected)
    ));
    assert!(sessions().is_empty());
    f.check_store(&original);
    let client = login(password).unwrap();
    client.resolve().unwrap();
    assert_eq!(sessions(), [id]);
    assert_eq!(client.read(f.board).unwrap(), original);
    f.check_store(&original);
    let mut saved = original;
    saved[1].title = "Coral".into();
    saved[1].revision = 2;
    assert_eq!(rename(&client, &f, 1).unwrap(), saved[1]);
    f.check_store(&saved);
    f.connect()
        .execute("DELETE FROM account_sessions WHERE account_id=$1", &[&id])
        .unwrap();
    assert_eq!(client.resolve(), Err(EditError::SessionRejected));
    denied(&client, &f, EditError::SessionRejected, &saved);
}

#[test]
fn viewer_reads_but_cannot_rename() {
    let f = Fixture::start();
    let _creator = Caller::board_creator(&f);
    let viewer = Caller::sign_in(&f, Role::Viewer);
    f.connect()
        .execute(
            "INSERT INTO board_subscribers(board_id,account_id) VALUES ($1,$2)",
            &[&f.board, &viewer.id],
        )
        .unwrap();
    let client = viewer.client(f.host.port);
    let original = rows("Review", 1);
    let read = client.read_with_access(f.board).unwrap();
    assert!(!read.can_edit);
    assert_eq!(read.rows, original);
    assert_eq!(rename(&client, &f, 1), Err(EditError::PermissionDenied));
    f.check_store(&original);
    revoke(&f.schema.config, &viewer.token).unwrap();
    denied(&client, &f, EditError::SessionRejected, &original);
    denied(
        &EditClient::new(f.host.port),
        &f,
        EditError::SessionRejected,
        &original,
    );
}

#[test]
fn member_edits_within_scope() {
    let f = Fixture::start();
    let member = Caller::board_creator(&f);
    let original = rows("Review", 1);
    let committed = rows("Coral", 2);
    let client = member.client(f.host.port);
    let read = client.read_with_access(f.board).unwrap();
    assert!(read.can_edit);
    assert_eq!(read.rows, original);
    assert_eq!(
        rename(&client, &f, original[1].revision).unwrap(),
        committed[1]
    );
    assert_eq!(member.client(f.host.port).read(f.board).unwrap(), committed);
    f.check_store(&committed);
    let outsider = Caller::sign_in(&f, Role::Member);
    // A real subscription on a different board is not transferable.
    let other_board = uuid::Uuid::new_v4();
    let mut db = f.connect();
    db.execute("INSERT INTO boards(id) VALUES ($1)", &[&other_board])
        .unwrap();
    db.execute(
        "INSERT INTO board_policies(board_id,creator_id) VALUES ($1,$2)",
        &[&other_board, &member.id],
    )
    .unwrap();
    db.execute(
        "INSERT INTO board_subscribers(board_id,account_id) VALUES ($1,$2)",
        &[&other_board, &outsider.id],
    )
    .unwrap();
    denied(
        &outsider.client(f.host.port),
        &f,
        EditError::PermissionDenied,
        &committed,
    );
    f.connect()
        .batch_execute("ALTER TABLE account_sessions RENAME TO unavailable_sessions")
        .unwrap();
    denied(&client, &f, EditError::AccessUnavailable, &committed);
    f.connect()
        .batch_execute("ALTER TABLE unavailable_sessions RENAME TO account_sessions")
        .unwrap();
    f.connect()
        .execute(
            "UPDATE accounts SET role='viewer' WHERE id=$1",
            &[&member.id],
        )
        .unwrap();
    let read = client.read_with_access(f.board).unwrap();
    assert!(!read.can_edit);
    assert_eq!(read.rows, committed);
    assert_eq!(
        client.rename(
            f.board,
            SECOND,
            &NameEdit {
                title: "Another name".into(),
                revision: 2
            }
        ),
        Err(EditError::PermissionDenied)
    );
    f.check_store(&committed);
    f.connect()
        .execute("DELETE FROM board_policies WHERE board_id=$1", &[&f.board])
        .unwrap();
    denied(&client, &f, EditError::PermissionDenied, &committed);
}
