mod browser;
#[allow(dead_code)]
mod fixture;
#[path = "fixture/peer.rs"]
mod peer;
#[allow(dead_code)]
#[path = "fixture/selection.rs"]
mod selection;
#[allow(dead_code)]
#[path = "fixture/stored_session.rs"]
mod stored_session;

use boardcraft_mobile_edit_client::Credentials;
use browser::Preview;
use stored_session::{StoredSession, assert_no_session};

#[test]
fn browser_reject_correct_list_logout() {
    let mut app = Preview::open(false);
    app.act("reject");
    assert_no_session(&app.f);
    app.act("login");
    StoredSession::only(&app.f, app.viewer);
    let peer = peer::Peer::sign_in(
        &app.f,
        app.viewer,
        Credentials {
            identifier: "reader".into(),
            password: app.password.clone(),
        },
    );
    let mut db = app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.batch_execute("LOCK TABLE account_sessions IN SHARE MODE")
        .unwrap();
    app.act("failed logout");
    peer.check(&app.f, true);
    lock.rollback().unwrap();
    app.act("logout");
    peer.check(&app.f, false);
    peer.client.resolve().unwrap();
    app.catalog.check(&app.f, true);
}

#[test]
fn browser_empty_unavailable_then_retry() {
    let mut app = Preview::open(false);
    app.act("login");
    let session = StoredSession::only(&app.f, app.viewer);
    let check = |app: &Preview, joined| {
        assert!(StoredSession::only(&app.f, app.viewer) == session);
        app.catalog.check(&app.f, joined);
    };
    app.catalog.membership(&app.f, false);
    app.act("empty");
    check(&app, false);
    app.catalog.membership(&app.f, true);
    let mut db = app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.batch_execute("LOCK TABLE board_policies IN ACCESS EXCLUSIVE MODE")
        .unwrap();
    app.act("unavailable");
    lock.rollback().unwrap();
    check(&app, true);
    app.act("retry");
    check(&app, true);
    session.revoke(&app.f);
    app.act("expired");
    assert_no_session(&app.f);
}
