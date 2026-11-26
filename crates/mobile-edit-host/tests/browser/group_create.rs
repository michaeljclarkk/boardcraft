use crate::{
    ReorderBoard,
    group_create::{self, BLUE, FAULT, GREEN},
    grouped_board,
    reorder_state::Snapshot,
    selection::SELECTED,
};
use boardcraft_board::{BoardId, store};
use uuid::Uuid;

fn open() -> ReorderBoard {
    let (app, session) = grouped_board(boardcraft_account::Role::Owner);
    group_create::seed(&app.f);
    let state = Snapshot::read(&app.f);
    let mut c = ReorderBoard {
        app,
        session,
        state,
    };
    c.batch_unchanged("new group open", "owner");
    c
}
fn unchanged(c: &ReorderBoard, role: &str) {
    assert_eq!(c.state, Snapshot::read(&c.app.f));
    c.access_checkpoint(role);
}
pub fn default_group() {
    let mut c = open();
    c.batch_unchanged("new group cancel", "owner");
    c.batch_unchanged("new group dialog", "owner");
    let mut db = c.app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.query_one(
        "SELECT revision FROM boards WHERE id=$1 FOR UPDATE",
        &[&SELECTED],
    )
    .unwrap();
    let a = format!("g-{}", c.app.proposed_item("new group pending"));
    unchanged(&c, "owner");
    lock.rollback().unwrap();
    c.app.act("new group committed");
    c.state = c.state.created_group(&c.app.f, &a, "New group", BLUE, 3);
    c.access_checkpoint("owner");
    c.batch_unchanged("new group reload default", "owner");
}
pub fn review() {
    let mut c = open();
    c.batch_unchanged("new group dialog", "owner");
    let a = format!("g-{}", c.app.proposed_item("new group default"));
    c.state = c.state.created_group(&c.app.f, &a, "New group", BLUE, 3);
    c.access_checkpoint("owner");
    c.batch_unchanged("new group edit", "owner");
    let mut db = c.app.f.connect();
    db.batch_execute(FAULT).unwrap();
    let b = format!("g-{}", c.app.proposed_item("new group fault"));
    assert_ne!(a, b);
    unchanged(&c, "owner");
    db.batch_execute("DROP TRIGGER refuse_group ON board_groups")
        .unwrap();
    let peer = format!("g-{}", Uuid::from_u128(800));
    let mut tx = db.transaction().unwrap();
    store::create_group(
        &mut tx,
        BoardId::new(SELECTED).unwrap(),
        &peer,
        2,
        "Peer",
        BLUE,
    )
    .ok()
    .unwrap();
    tx.commit().unwrap();
    c.state = c.state.created_group(&c.app.f, &peer, "Peer", BLUE, 4);
    c.access_checkpoint("owner");
    for action in ["new group conflict", "new group review", "new group adopt"] {
        c.batch_unchanged(action, "owner");
    }
    c.app.act("new group refresh fault");
    c.state = c.state.created_group(&c.app.f, &b, "Work", GREEN, 5);
    c.access_checkpoint("owner");
    for action in ["new group read accepted", "new group reload green"] {
        c.batch_unchanged(action, "owner");
    }
}
pub fn demote() {
    let mut c = open();
    c.batch_unchanged("new group dialog", "owner");
    for role in ["member", "viewer"] {
        c.app
            .f
            .connect()
            .execute(
                "UPDATE accounts SET role=$2 WHERE id=$1",
                &[&c.app.account, &role],
            )
            .unwrap();
        if role == "member" {
            c.batch_unchanged("new group denied", role);
        }
        c.batch_unchanged(&format!("new group {role}"), role);
    }
    c.app.catalog.membership(&c.app.f, false);
    c.app.act("new group withdrawn");
    c.app.catalog.check_grants(&c.app.f, false);
    assert!(c.session.present(&c.app.f));
    assert_eq!(Snapshot::read(&c.app.f), c.state);
    c.session.revoke(&c.app.f);
    c.app.act("new group expired");
    c.app.catalog.check_grants(&c.app.f, false);
    assert!(!c.session.present(&c.app.f));
    assert_eq!(Snapshot::read(&c.app.f), c.state);
}
