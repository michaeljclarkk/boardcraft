mod browser;
#[path = "fixture/group_create.rs"]
mod group_create;
#[path = "browser/group_create.rs"]
mod group_create_browser;
#[path = "fixture/group_edit.rs"]
mod group_edit;
#[path = "browser/group_edit.rs"]
mod group_edit_browser;
#[path = "browser/group_order.rs"]
mod group_order_browser;

#[test]
fn browser_drags_group_across_sections() {
    group_order_browser::across();
}
#[test]
fn browser_cancels_group_heading_drag() {
    group_order_browser::cancel_drag();
}
#[test]
fn browser_scrolls_to_group_drop_target() {
    group_order_browser::scroll();
}
#[test]
fn browser_recovers_group_drag_after_refusal() {
    group_order_browser::recover_drag();
}

#[test]
fn browser_moves_group_one_place_from_menu() {
    group_order_browser::moves();
}
#[test]
fn browser_reloads_order_before_moving_group_again() {
    group_order_browser::recovery();
}
#[test]
fn browser_demoted_owner_cannot_reorder_groups() {
    group_order_browser::demote();
}

#[test]
fn browser_edits_existing_group_by_id() {
    group_edit_browser::edit();
}
#[test]
fn browser_reviews_conflicting_group_edit() {
    group_edit_browser::review();
}
#[test]
fn browser_demoted_owner_cannot_edit_group() {
    group_edit_browser::demote();
}

#[test]
fn browser_creates_duplicate_group_in_position_order() {
    group_create_browser::default_group();
}
#[test]
fn browser_reviews_failed_group_create() {
    group_create_browser::review();
}
#[test]
fn browser_demoted_owner_cannot_create_group() {
    group_create_browser::demote();
}
#[allow(dead_code)]
mod fixture;
#[path = "fixture/lifecycle.rs"]
mod lifecycle;
#[path = "fixture/move_state.rs"]
mod move_state;
#[path = "fixture/peer.rs"]
mod peer;
#[path = "fixture/reorder_state.rs"]
mod reorder_state;
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

struct MoveBoard {
    app: Preview,
    session: StoredSession,
    state: move_state::MoveState,
}

struct ReorderBoard {
    app: Preview,
    session: StoredSession,
    state: reorder_state::Snapshot,
}
impl ReorderBoard {
    fn group_owner() -> Self {
        let (mut app, session) = grouped_board(boardcraft_account::Role::Owner);
        let state = lifecycle::seed_group_recovery(&app.f, app.account);
        app.act("group life open");
        let c = Self {
            app,
            state,
            session,
        };
        assert_eq!(c.state, reorder_state::Snapshot::read(&c.app.f));
        c.access_checkpoint("owner");
        c
    }
    fn group_changed(&mut self, step: &str, active: bool) {
        self.app.act(step);
        self.state = self.state.group_transition(&self.app.f, "g-90", active);
        self.access_checkpoint("owner");
    }
    fn owner_board() -> Self {
        let (mut app, session) = grouped_board(boardcraft_account::Role::Owner);
        lifecycle::seed(&app.f, app.account);
        let state = reorder_state::Snapshot::read(&app.f);
        app.act("select member");
        Self {
            app,
            session,
            state,
        }
    }
    fn changed(&mut self, step: &str, action: &str) {
        self.app.act(step);
        self.state = self.state.lifecycle(&self.app.f, self.app.account, action);
        self.access_checkpoint("owner");
    }
    fn batch_dialog() -> Self {
        let mut c = Self::open_as(boardcraft_account::Role::Member, "select member");
        c.checkpoint("batch open", 0, "member");
        c
    }
    fn batch_unchanged(&mut self, action: &str, role: &str) {
        self.app.act(action);
        assert_eq!(reorder_state::Snapshot::read(&self.app.f), self.state);
        self.access_checkpoint(role);
    }
    fn open() -> Self {
        Self::open_as(boardcraft_account::Role::Member, "reorder open")
    }
    fn open_as(role: boardcraft_account::Role, entry: &str) -> Self {
        let (mut app, session) = grouped_board(role);
        reorder_state::seed(&app.f, app.account);
        let state = reorder_state::Snapshot::read(&app.f);
        app.act(entry);
        Self {
            app,
            session,
            state,
        }
    }
    fn checkpoint(&mut self, action: &str, drops: usize, role: &str) {
        self.app.act(action);
        self.state.check(&self.app.f, self.app.account, drops);
        self.access_checkpoint(role);
    }
    fn access_checkpoint(&self, role: &str) {
        self.app.catalog.check_grants(&self.app.f, true);
        assert!(StoredSession::only(&self.app.f, self.app.account) == self.session);
        assert_eq!(
            self.app
                .f
                .connect()
                .query_one(
                    "SELECT role FROM accounts WHERE id=$1",
                    &[&self.app.account]
                )
                .unwrap()
                .get::<_, String>(0),
            role
        );
    }
}

#[test]
fn browser_archives_and_restores_one_group() {
    let mut c = ReorderBoard::group_owner();
    c.batch_unchanged("group life escape", "owner");
    let mut db = c.app.f.connect();
    let mut tx = db.transaction().unwrap();
    tx.query_one(
        "SELECT revision FROM boards WHERE id=$1 FOR UPDATE",
        &[&selection::SELECTED],
    )
    .unwrap();
    c.batch_unchanged("group life pending", "owner");
    tx.rollback().unwrap();
    c.group_changed("group life committed", false);
    c.group_changed("group life undo", true);
    c.group_changed("group life again", false);
    for step in [
        "group life dismiss",
        "group life inspect",
        "group life back",
        "group life forward",
        "group life reload recovery",
    ] {
        c.batch_unchanged(step, "owner");
    }
    c.group_changed("group life restore", true);
    c.batch_unchanged("group life close", "owner");
}

#[test]
fn browser_recovers_failed_group_lifecycle() {
    let mut c = ReorderBoard::group_owner();
    let mut db = c.app.f.connect();
    db.batch_execute("CREATE FUNCTION fail_group() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture group fault'; END $$; CREATE CONSTRAINT TRIGGER fail_group AFTER UPDATE ON board_groups DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_group()").unwrap();
    c.batch_unchanged("group life fault", "owner");
    db.batch_execute("DROP TRIGGER fail_group ON board_groups")
        .unwrap();
    c.group_changed("group life archive", false);
    c.batch_unchanged("group life inspect", "owner");
    db.execute(
        "UPDATE boards SET revision=4 WHERE id=$1",
        &[&selection::SELECTED],
    )
    .unwrap();
    c.state = reorder_state::Snapshot::read(&c.app.f);
    for step in ["group life stale", "group life review"] {
        c.batch_unchanged(step, "owner");
    }
    c.group_changed("group life refresh fault", true);
    c.batch_unchanged("group life read accepted", "owner");
}

#[test]
fn browser_demoted_owner_cannot_restore_group() {
    let mut c = ReorderBoard::group_owner();
    c.group_changed("group life archive", false);
    c.batch_unchanged("group life inspect", "owner");
    let mut db = c.app.f.connect();
    for (role, steps) in [
        (
            "member",
            vec![
                "group life denied",
                "group life readonly",
                "group life member",
                "group life inspect readonly",
            ],
        ),
        ("viewer", vec!["group life readonly", "group life viewer"]),
    ] {
        db.execute(
            "UPDATE accounts SET role=$2 WHERE id=$1",
            &[&c.app.account, &role],
        )
        .unwrap();
        for step in steps {
            c.batch_unchanged(step, role);
        }
    }
    c.app.catalog.membership(&c.app.f, false);
    c.app.act("life unavailable");
    assert_eq!(c.state, reorder_state::Snapshot::read(&c.app.f));
    c.app.catalog.check_grants(&c.app.f, false);
    assert!(c.session.present(&c.app.f));
    c.session.revoke(&c.app.f);
    c.app.act("life expired");
    assert_eq!(c.state, reorder_state::Snapshot::read(&c.app.f));
    c.app.catalog.check_grants(&c.app.f, false);
    assert!(!c.session.present(&c.app.f));
}

#[test]
fn browser_archives_and_restores_one_root() {
    let mut c = ReorderBoard::owner_board();
    for step in ["life cancel", "life dialog"] {
        c.batch_unchanged(step, "owner");
    }
    let mut db = c.app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.query_one(
        "SELECT revision FROM boards WHERE id=$1 FOR UPDATE",
        &[&selection::SELECTED],
    )
    .unwrap();
    c.batch_unchanged("life pending", "owner");
    lock.rollback().unwrap();
    c.changed("life committed", "archive");
    for step in ["life inspect", "life reload"] {
        c.batch_unchanged(step, "owner");
    }
    let mut lock = db.transaction().unwrap();
    lock.query_one(
        "SELECT revision FROM boards WHERE id=$1 FOR UPDATE",
        &[&selection::SELECTED],
    )
    .unwrap();
    c.batch_unchanged("life restoring", "owner");
    lock.rollback().unwrap();
    c.changed("life restored", "restore");
    for step in ["life close", "life empty"] {
        c.batch_unchanged(step, "owner");
    }
}

#[test]
fn browser_recovers_failed_root_lifecycle() {
    let mut c = ReorderBoard::owner_board();
    c.batch_unchanged("life dialog", "owner");
    let mut db = c.app.f.connect();
    db.batch_execute("CREATE FUNCTION fail_receipt() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture commit fault'; END $$; CREATE CONSTRAINT TRIGGER fail_receipt AFTER INSERT ON group_reorder_receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_receipt()").unwrap();
    c.batch_unchanged("life fault", "owner");
    db.batch_execute("DROP TRIGGER fail_receipt ON group_reorder_receipts")
        .unwrap();
    c.changed("life retry", "archive");
    c.batch_unchanged("life inspect", "owner");
    db.execute(
        "UPDATE boards SET revision=3 WHERE id=$1",
        &[&selection::SELECTED],
    )
    .unwrap();
    c.state = reorder_state::Snapshot::read(&c.app.f);
    for step in ["life stale", "life review"] {
        c.batch_unchanged(step, "owner");
    }
    c.changed("life latest restore", "restore");
    c.batch_unchanged("life close", "owner");
    c.batch_unchanged("life dialog", "owner");
    db.execute(
        "UPDATE board_items SET revision=revision+1 WHERE id=$1",
        &[&uuid::Uuid::from_u128(67)],
    )
    .unwrap();
    c.state = reorder_state::Snapshot::read(&c.app.f);
    for step in ["life stale archive", "life review archive"] {
        c.batch_unchanged(step, "owner");
    }
}

#[test]
fn browser_demoted_member_cannot_restore_root() {
    let mut c = ReorderBoard::owner_board();
    c.batch_unchanged("life dialog", "owner");
    c.changed("life archive", "archive");
    c.batch_unchanged("life inspect", "owner");
    let mut db = c.app.f.connect();
    db.execute(
        "UPDATE accounts SET role='member' WHERE id=$1",
        &[&c.app.account],
    )
    .unwrap();
    for step in [
        "life denied",
        "life readonly",
        "life member grid",
        "life return readonly",
    ] {
        c.batch_unchanged(step, "member");
    }
    db.execute(
        "UPDATE accounts SET role='viewer' WHERE id=$1",
        &[&c.app.account],
    )
    .unwrap();
    for step in ["life readonly", "life viewer grid"] {
        c.batch_unchanged(step, "viewer");
    }
    c.app.catalog.membership(&c.app.f, false);
    c.app.act("life unavailable");
    assert_eq!(c.state, reorder_state::Snapshot::read(&c.app.f));
    c.app.catalog.check_grants(&c.app.f, false);
    assert!(c.session.present(&c.app.f));
    c.session.revoke(&c.app.f);
    c.app.act("life expired");
    assert_eq!(c.state, reorder_state::Snapshot::read(&c.app.f));
    c.app.catalog.check_grants(&c.app.f, false);
    assert!(!c.session.present(&c.app.f));
}

#[test]
fn browser_moves_toolbar_selection_in_request_order() {
    let mut c = ReorderBoard::batch_dialog();
    c.batch_unchanged("batch cancel", "member");
    let mut db = c.app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.query_one(
        "SELECT revision FROM boards WHERE id=$1 FOR UPDATE",
        &[&selection::SELECTED],
    )
    .unwrap();
    c.batch_unchanged("batch pending", "member");
    lock.rollback().unwrap();
    let order = [73, 77, 67, 59, 61];
    c.app.act("batch saved");
    c.state = c
        .state
        .batch(&c.app.f, c.app.account, "g-90", &order, &[67, 59, 61]);
    c.access_checkpoint("member");
    c.batch_unchanged("batch reload", "member");
    c.app.act("batch reserve");
    c.state
        .batch(&c.app.f, c.app.account, "g-10", &order, &order);
    c.access_checkpoint("member");
}

#[test]
fn browser_reviews_conflicting_selection_move() {
    let mut c = ReorderBoard::batch_dialog();
    let mut db = c.app.f.connect();
    let id = uuid::Uuid::from_u128(61);
    db.execute(
        "UPDATE board_items SET title='Indigo',revision=2 WHERE id=$1",
        &[&id],
    )
    .unwrap();
    c.state = reorder_state::Snapshot::read(&c.app.f);
    c.batch_unchanged("batch conflict", "member");
    db.execute("UPDATE board_items SET group_id='g-30' WHERE id=$1", &[&id])
        .unwrap();
    let visible = c.state.clone();
    c.state = reorder_state::Snapshot::read(&c.app.f);
    c.batch_unchanged("batch unavailable", "member");
    db.execute("UPDATE board_items SET group_id='g-90' WHERE id=$1", &[&id])
        .unwrap();
    c.state = visible;
    c.batch_unchanged("batch review", "member");
    db.batch_execute("CREATE FUNCTION fail_batch() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture commit fault'; END $$; CREATE CONSTRAINT TRIGGER fail_batch AFTER INSERT ON group_reorder_receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_batch()").unwrap();
    c.batch_unchanged("batch fault", "member");
    db.batch_execute("DROP TRIGGER fail_batch ON group_reorder_receipts")
        .unwrap();
    c.app.act("batch reviewed saved");
    c.state.batch(
        &c.app.f,
        c.app.account,
        "g-90",
        &[73, 77, 67, 59, 61],
        &[67, 59, 61],
    );
    c.access_checkpoint("member");
}

#[test]
fn browser_demoted_viewer_cannot_move_selection() {
    let mut c = ReorderBoard::batch_dialog();
    c.batch_unchanged("batch choose other", "member");
    c.app
        .f
        .connect()
        .execute(
            "UPDATE accounts SET role='viewer' WHERE id=$1",
            &[&c.app.account],
        )
        .unwrap();
    c.batch_unchanged("batch denied", "viewer");
    c.batch_unchanged("batch viewer", "viewer");
}

#[test]
fn browser_selects_duplicate_rows_and_groups() {
    let mut c = ReorderBoard::open_as(boardcraft_account::Role::Member, "select member");
    c.checkpoint("select duplicates", 0, "member");
}

#[test]
fn browser_shift_selects_roots_across_groups() {
    let mut c = ReorderBoard::open_as(boardcraft_account::Role::Viewer, "select viewer");
    c.checkpoint("select reverse", 0, "viewer");
    c.checkpoint("select reload", 0, "viewer");
    let mut db = c.app.f.connect();
    let mut lock = db.transaction().unwrap();
    // Blocks the authorized read's FOR SHARE, but permits independent readbacks.
    lock.batch_execute("LOCK TABLE account_sessions IN EXCLUSIVE MODE")
        .unwrap();
    c.checkpoint("select failed", 0, "viewer");
    lock.rollback().unwrap();
    c.checkpoint("select retry", 0, "viewer");
}

#[test]
fn browser_reorders_duplicate_rows_by_pointer() {
    let mut c = ReorderBoard::open();
    c.checkpoint("reorder cancel", 0, "member");
    c.checkpoint("reorder mark", 0, "member");
    let mut db = c.app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.query_one(
        "SELECT revision FROM boards WHERE id=$1 FOR UPDATE",
        &[&selection::SELECTED],
    )
    .unwrap();
    c.checkpoint("reorder pending", 0, "member");
    lock.rollback().unwrap();
    for (action, n) in [
        ("reorder committed", 1),
        ("reorder before reload", 1),
        ("reorder after", 2),
        ("reorder after reload", 2),
    ] {
        c.checkpoint(action, n, "member");
    }
}
#[test]
fn browser_demoted_viewer_cannot_reorder() {
    let mut c = ReorderBoard::open();
    c.checkpoint("reorder mark", 0, "member");
    c.app
        .f
        .connect()
        .execute(
            "UPDATE accounts SET role='viewer' WHERE id=$1",
            &[&c.app.account],
        )
        .unwrap();
    for a in ["reorder denied", "reorder viewer reload"] {
        c.checkpoint(a, 0, "viewer");
    }
}
#[test]
fn browser_failed_reorder_rolls_back_then_redrags() {
    let mut c = ReorderBoard::open();
    let mut db = c.app.f.connect();
    db.batch_execute("CREATE FUNCTION fail_receipt() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture commit fault'; END $$; CREATE CONSTRAINT TRIGGER fail_receipt AFTER INSERT ON group_reorder_receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_receipt()").unwrap();
    c.checkpoint("reorder mark", 0, "member");
    c.checkpoint("reorder failed", 0, "member");
    db.batch_execute("DROP TRIGGER fail_receipt ON group_reorder_receipts")
        .unwrap();
    c.checkpoint("reorder initial reload", 0, "member");
    c.checkpoint("reorder before", 1, "member");
    c.checkpoint("reorder before reload", 1, "member");
}
impl MoveBoard {
    fn open() -> Self {
        let (mut app, session) = grouped_board(boardcraft_account::Role::Member);
        let state = move_state::MoveState::seed(&app.f, app.account);
        app.act("groups member");
        Self {
            app,
            session,
            state,
        }
    }
    fn check(&self, moved: bool, role: &str) {
        let f = &self.app.f;
        assert!(StoredSession::only(f, self.app.account) == self.session);
        self.app.catalog.check_grants(f, true);
        assert_eq!(
            f.connect()
                .query_one(
                    "SELECT role FROM accounts WHERE id=$1",
                    &[&self.app.account]
                )
                .unwrap()
                .get::<_, String>(0),
            role
        );
        self.state.groups(f, false);
        let (group, rev) = if moved { ("g-20", 2) } else { ("g-90", 1) };
        self.state.check(f, group, "Ink", rev, rev);
        let board = boardcraft_board::BoardId::new(selection::SELECTED).unwrap();
        let flat = boardcraft_board::store::load_items(&mut f.connect(), board)
            .unwrap()
            .unwrap();
        assert_eq!(
            flat.iter()
                .map(|r| r.id.as_uuid().as_u128())
                .collect::<Vec<_>>(),
            [73, 61, 59]
        );
    }
}

#[test]
fn browser_moves_item_by_group_id() {
    let mut case = MoveBoard::open();
    case.app.act("move cancel");
    case.check(false, "member");
    case.app.act("move choose");
    let mut db = case.app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.query_one(
        "SELECT id FROM board_groups WHERE board_id=$1 AND id='g-20' FOR UPDATE",
        &[&selection::SELECTED],
    )
    .unwrap();
    case.app.act("move pending");
    case.check(false, "member");
    lock.rollback().unwrap();
    for action in ["move saved", "move reload"] {
        case.app.act(action);
        case.check(true, "member");
    }
}

#[test]
fn browser_demoted_viewer_cannot_move_item() {
    let mut case = MoveBoard::open();
    case.app.act("move choose");
    case.app
        .f
        .connect()
        .execute(
            "UPDATE accounts SET role='viewer' WHERE id=$1",
            &[&case.app.account],
        )
        .unwrap();
    for action in ["move denied", "move viewer reload"] {
        case.app.act(action);
        case.check(false, "viewer");
    }
}

#[test]
fn browser_reviews_failed_group_move() {
    let mut case = MoveBoard::open();
    let mut db = case.app.f.connect();
    db.batch_execute("CREATE FUNCTION reject_move() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture move fault'; END $$;
        CREATE CONSTRAINT TRIGGER fail_commit AFTER UPDATE ON board_items DEFERRABLE INITIALLY DEFERRED FOR EACH ROW WHEN (OLD.group_id <> NEW.group_id) EXECUTE FUNCTION reject_move()").unwrap();
    for action in [
        "move choose",
        "move failed",
        "move review",
        "move dismiss",
        "move choose",
        "move failed",
    ] {
        case.app.act(action);
        case.check(false, "member");
    }
    db.batch_execute("DROP TRIGGER fail_commit ON board_items; DROP FUNCTION reject_move()")
        .unwrap();
    case.app.act("move review");
    case.check(false, "member");
    for action in ["move apply", "move reload"] {
        case.app.act(action);
        case.check(true, "member");
    }
    // A peer changes the source before the menu opens, then again after the
    // explicit review. Neither the initial refresh nor stale Apply may win.
    let peer_move = |db: &mut postgres::Client, group: &str| {
        db.execute("UPDATE board_items SET group_id=$2,revision=revision+1,group_revision=revision+1 WHERE id=$1", &[&uuid::Uuid::from_u128(61),&group]).unwrap();
    };
    peer_move(&mut db, "g-90");
    for action in ["move changed before open", "move review"] {
        case.app.act(action);
        case.state.check(&case.app.f, "g-90", "Ink", 3, 3);
    }
    peer_move(&mut db, "g-20");
    case.app.act("move apply changed review");
    case.state.check(&case.app.f, "g-20", "Ink", 4, 4);
    case.state.groups(&case.app.f, false);
    case.app.catalog.check_grants(&case.app.f, true);
    assert!(StoredSession::only(&case.app.f, case.app.account) == case.session);
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

fn creation_checkpoint(
    app: &Preview,
    session: &StoredSession,
    role: &str,
    appended: Option<uuid::Uuid>,
) {
    assert!(StoredSession::only(&app.f, app.account) == *session);
    assert_eq!(
        app.f
            .connect()
            .query_one("SELECT role FROM accounts WHERE id=$1", &[&app.account])
            .unwrap()
            .get::<_, String>(0),
        role
    );
    app.catalog.check_grants(&app.f, true);
    selection::check_group_items(
        &app.f,
        false,
        appended.map(|id| (id, "g-20", 1, "Ink", app.account)),
    );
}

#[test]
fn browser_creates_item_in_selected_group() {
    let (mut app, session) = grouped_board(boardcraft_account::Role::Member);
    app.act("groups member");
    app.act("create blank and cancel");
    creation_checkpoint(&app, &session, "member", None);
    let id = app.proposed_item("create rejected");
    assert!(
        app.f
            .connect()
            .query_opt("SELECT id FROM board_items WHERE id=$1", &[&id])
            .unwrap()
            .is_none()
    );
    creation_checkpoint(&app, &session, "member", None);
    app.act("create corrected");
    let mut db = app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.query_one(
        "SELECT id FROM board_groups WHERE board_id=$1 AND id='g-20' FOR UPDATE",
        &[&selection::SELECTED],
    )
    .unwrap();
    app.act("create pending");
    creation_checkpoint(&app, &session, "member", None);
    lock.rollback().unwrap();
    for action in ["create saved", "create reload"] {
        app.act(action);
        creation_checkpoint(&app, &session, "member", Some(id));
    }
}

#[test]
fn browser_demoted_viewer_cannot_create_item() {
    let (mut app, session) = grouped_board(boardcraft_account::Role::Member);
    app.act("groups member");
    app.act("create member draft");
    creation_checkpoint(&app, &session, "member", None);
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
    let id = app.proposed_item("create denied");
    assert!(
        app.f
            .connect()
            .query_opt("SELECT id FROM board_items WHERE id=$1", &[&id])
            .unwrap()
            .is_none()
    );
    creation_checkpoint(&app, &session, "viewer", None);
    app.act("create viewer reload");
    creation_checkpoint(&app, &session, "viewer", None);
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
