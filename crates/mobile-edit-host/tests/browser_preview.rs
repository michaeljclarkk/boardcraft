mod browser;
#[allow(dead_code)]
mod fixture;
#[path = "fixture/peer.rs"]
mod peer;
#[path = "browser/resume.rs"]
mod resume;
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
fn browser_reload_and_independent_tab_resume() {
    resume::two_tabs();
}

#[test]
fn browser_unavailable_then_rejected_resume() {
    resume::resolve_failure();
}

fn check_catalog(app: &Preview, session: &StoredSession, joined: bool) {
    assert!(StoredSession::only(&app.f, app.account) == *session);
    app.catalog.check(&app.f, joined);
}

fn grouped_board(role: boardcraft_account::Role) -> (Preview, StoredSession) {
    let mut app = Preview::with_role(false, "open", role);
    assert_no_session(&app.f);
    app.act("login");
    let session = StoredSession::only(&app.f, app.account);
    selection::seed_groups(&app.f);
    (app, session)
}

fn grouped_checkpoint(app: &Preview, session: &StoredSession, joined: bool, saved: bool) {
    assert!(StoredSession::only(&app.f, app.account) == *session);
    app.catalog.check_grants(&app.f, joined);
    selection::check_groups(&app.f, saved);
}

#[test]
fn browser_opens_and_edits_grouped_board() {
    let (mut app, session) = grouped_board(boardcraft_account::Role::Member);
    for (action, saved) in [
        ("groups member", false),
        ("groups save", true),
        ("groups reload", true),
        ("back saved", true),
        ("groups sibling", true),
    ] {
        app.act(action);
        grouped_checkpoint(&app, &session, true, saved);
    }
}

#[test]
fn browser_grouped_board_rechecks_membership() {
    let (mut app, session) = grouped_board(boardcraft_account::Role::Viewer);
    app.act("groups viewer");
    grouped_checkpoint(&app, &session, true, false);
    app.catalog.membership(&app.f, false);
    app.act("groups denied");
    grouped_checkpoint(&app, &session, false, false);
    app.catalog.membership(&app.f, true);
    app.act("groups retry");
    grouped_checkpoint(&app, &session, true, false);
}

#[test]
fn browser_direct_board_survives_sign_in() {
    let mut app = Preview::start(false, "open direct");
    assert_no_session(&app.f);
    app.catalog.check(&app.f, true);
    for action in ["direct identifier", "direct rejection"] {
        app.act(action);
        assert_no_session(&app.f);
        app.catalog.check(&app.f, true);
    }
    app.act("direct login");
    let session = StoredSession::only(&app.f, app.account);
    check_catalog(&app, &session, true);
    app.act("back");
    check_catalog(&app, &session, true);
}

#[test]
fn browser_history_rechecks_board_access() {
    let mut app = Preview::open(false);
    assert_no_session(&app.f);
    app.act("login");
    let session = StoredSession::only(&app.f, app.account);
    check_catalog(&app, &session, true);
    for action in ["open last", "history back", "forward", "history back"] {
        app.act(action);
        check_catalog(&app, &session, true);
    }
    app.catalog.membership(&app.f, false);
    app.act("forward denied");
    check_catalog(&app, &session, false);
    app.catalog.membership(&app.f, true);
    app.act("retry restored");
    check_catalog(&app, &session, true);

    // Traverse away before the selected read can finish. Its successful reply
    // must not put even hidden item rows back into the list document.
    app.act("history back");
    check_catalog(&app, &session, true);
    let mut db = app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.batch_execute("LOCK TABLE board_items IN ACCESS EXCLUSIVE MODE")
        .unwrap();
    app.act("forward then back pending");
    lock.rollback().unwrap();
    app.act("settled list");
    check_catalog(&app, &session, true);
}

#[test]
fn browser_opens_selected_board_by_id() {
    let mut app = Preview::open(false);
    app.act("login");
    let session = StoredSession::only(&app.f, app.account);
    app.catalog.check(&app.f, true);
    for action in ["open last", "back", "open first review"] {
        app.act(action);
        check_catalog(&app, &session, true);
    }
}

#[test]
fn browser_revoked_membership_blocks_selection() {
    let mut app = Preview::open(false);
    app.act("login");
    let session = StoredSession::only(&app.f, app.account);
    app.catalog.check(&app.f, true);
    app.catalog.membership(&app.f, false);
    for action in ["denied", "retry denied", "back empty"] {
        app.act(action);
        check_catalog(&app, &session, false);
    }
    app.catalog.membership(&app.f, true);
    for action in ["refresh restored", "open last"] {
        app.act(action);
        check_catalog(&app, &session, true);
    }
}

#[test]
fn browser_reject_correct_list_logout() {
    let mut app = Preview::open(false);
    app.act("reject");
    assert_no_session(&app.f);
    app.act("login");
    StoredSession::only(&app.f, app.account);
    let peer = peer::Peer::sign_in(
        &app.f,
        app.account,
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
    let session = StoredSession::only(&app.f, app.account);
    app.catalog.membership(&app.f, false);
    app.act("empty");
    check_catalog(&app, &session, false);
    app.catalog.membership(&app.f, true);
    let mut db = app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.batch_execute("LOCK TABLE board_policies IN ACCESS EXCLUSIVE MODE")
        .unwrap();
    app.act("unavailable");
    lock.rollback().unwrap();
    check_catalog(&app, &session, true);
    app.act("retry");
    check_catalog(&app, &session, true);
    session.revoke(&app.f);
    app.act("expired");
    assert_no_session(&app.f);
}

fn check_name(app: &Preview, session: &StoredSession, role: &str, saved: bool) {
    use boardcraft_mobile_edit_client::SavedItem;
    assert!(StoredSession::only(&app.f, app.account) == *session);
    assert_eq!(
        app.f
            .connect()
            .query_one("SELECT role FROM accounts WHERE id=$1", &[&app.account])
            .unwrap()
            .get::<_, String>(0),
        role
    );
    let mut rows = selection::selected_rows();
    if saved {
        rows[1].title = "Coral".into();
        rows[1].revision = 2;
    }
    app.catalog.check_with_selected(&app.f, true, &rows);
    app.f
        .check_store(&[fixture::FIRST, fixture::SECOND].map(|id| SavedItem {
            id,
            title: "Review".into(),
            revision: 1,
        }));
}

fn member_board() -> (Preview, StoredSession) {
    let mut app = Preview::with_role(false, "open member", boardcraft_account::Role::Member);
    assert_no_session(&app.f);
    app.act("login");
    let session = StoredSession::only(&app.f, app.account);
    check_name(&app, &session, "member", false);
    app.act("open editable");
    check_name(&app, &session, "member", false);
    (app, session)
}

#[test]
fn browser_member_recovers_selected_name() {
    let (mut app, session) = member_board();
    for action in [
        "cancel name",
        "unchanged name",
        "blank blur",
        "cancel rejected",
        "retry name",
        "dismiss name",
        "draft coral",
    ] {
        app.act(action);
        check_name(&app, &session, "member", false);
    }
    let mut db = app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.query_one(
        "SELECT id FROM board_items WHERE board_id=$1 AND id=$2 FOR UPDATE",
        &[&selection::SELECTED, &uuid::Uuid::from_u128(61)],
    )
    .unwrap();
    app.act("pending name");
    check_name(&app, &session, "member", false);
    lock.rollback().unwrap();
    for action in ["saved name", "back saved", "reopen saved"] {
        app.act(action);
        check_name(&app, &session, "member", true);
    }
}

#[test]
fn browser_demoted_viewer_cannot_rename() {
    let (mut app, session) = member_board();
    app.act("draft coral");
    check_name(&app, &session, "member", false);
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
    check_name(&app, &session, "viewer", false);
    for action in [
        "denied name",
        "back",
        "reopen readonly",
        "readonly attempts",
    ] {
        app.act(action);
        check_name(&app, &session, "viewer", false);
    }
}
