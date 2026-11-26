use super::*;

fn catalog(app: &Preview, joined: bool) {
    assert_eq!(
        app.f
            .connect()
            .query_one("SELECT role FROM accounts WHERE id=$1", &[&app.account])
            .unwrap()
            .get::<_, String>(0),
        "viewer"
    );
    app.catalog.check(&app.f, joined);
}

pub(super) fn two_tabs() {
    let (mut app, first) = member_board();
    assert_eq!(
        app.f
            .connect()
            .execute(
                "UPDATE accounts SET role='viewer' WHERE id=$1",
                &[&app.account]
            )
            .unwrap(),
        1
    );
    for (action, joined) in [
        ("reload demoted", true),
        ("open sibling tab", true),
        ("sibling denied", false),
        ("retry sibling", true),
    ] {
        if action == "sibling denied" || action == "retry sibling" {
            app.catalog.membership(&app.f, joined);
        }
        app.act(action);
        assert!(StoredSession::only(&app.f, app.account) == first);
        catalog(&app, joined);
    }
    // A second real preview on the same host sees port-shared cookies, but
    // has a separate binding store, cookie namespace and bearer table.
    let other = boardcraft_mobile_edit_host::Host::start_preview(
        &app.f.schema.config,
        boardcraft_board::BoardId::new(app.f.board).unwrap(),
    )
    .unwrap();
    app.open_peer(other.port);
    assert!(StoredSession::only(&app.f, app.account) == first);
    catalog(&app, true);
    app.act("peer login");
    let pair = StoredSession::all(&app.f, app.account);
    assert!(pair.len() == 2 && pair.contains(&first));
    let peer = pair.iter().find(|s| **s != first).unwrap();
    let check = |app: &Preview| {
        assert!(StoredSession::all(&app.f, app.account) == pair);
        catalog(app, true);
    };
    check(&app);
    app.act("back");
    check(&app);
    let mut db = app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.batch_execute("LOCK TABLE account_sessions IN SHARE MODE")
        .unwrap();
    for action in ["failed logout", "failed logout status", "sibling retained"] {
        app.act(action);
        check(&app);
    }
    lock.rollback().unwrap();
    for action in [
        "logout",
        "both signed out",
        "reload signed out",
        "peer survives",
    ] {
        app.act(action);
        assert!(StoredSession::only(&app.f, app.account) == *peer);
        catalog(&app, true);
    }
}

pub(super) fn resolve_failure() {
    let mut app = Preview::open(false);
    app.act("waiting sibling");
    assert_no_session(&app.f);
    catalog(&app, true);
    app.act("login");
    let first = StoredSession::only(&app.f, app.account);
    catalog(&app, true);
    app.act("sibling saw login");
    assert!(StoredSession::only(&app.f, app.account) == first);
    catalog(&app, true);
    app.act("open last");
    check_catalog(&app, &first, true);
    catalog(&app, true);
    let mut db = app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.batch_execute("LOCK TABLE account_sessions IN ACCESS EXCLUSIVE MODE")
        .unwrap();
    app.act("unavailable resume");
    lock.rollback().unwrap();
    check_catalog(&app, &first, true);
    catalog(&app, true);
    app.act("retry resume");
    check_catalog(&app, &first, true);
    catalog(&app, true);
    first.revoke(&app.f);
    app.act("rejected resume");
    assert_no_session(&app.f);
    catalog(&app, true);
    app.act("sibling saw rejection");
    assert_no_session(&app.f);
    catalog(&app, true);
    let mut lock = db.transaction().unwrap();
    lock.batch_execute("LOCK TABLE account_sessions IN ACCESS EXCLUSIVE MODE")
        .unwrap();
    app.act("forgotten resume");
    lock.rollback().unwrap();
    assert_no_session(&app.f);
    catalog(&app, true);
}
