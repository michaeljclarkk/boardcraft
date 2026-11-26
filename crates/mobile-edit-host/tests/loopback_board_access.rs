#[path = "fixture/caller.rs"]
mod caller;
mod fixture;
#[path = "fixture/navigation.rs"]
mod navigation;
#[path = "fixture/peer.rs"]
mod peer;
#[path = "fixture/stored_session.rs"]
mod stored_session;

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
