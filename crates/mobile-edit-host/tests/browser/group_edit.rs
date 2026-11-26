use crate::{
    ReorderBoard,
    group_create::{BLUE, GREEN},
    group_edit::{self, FAULT, STORED},
    grouped_board,
    reorder_state::Snapshot,
    selection::SELECTED,
};
use boardcraft_board::{BoardId, store};

pub(super) fn open() -> ReorderBoard {
    let (app, session) = grouped_board(boardcraft_account::Role::Owner);
    group_edit::seed(&app.f);
    let state = Snapshot::read(&app.f);
    let mut c = ReorderBoard {
        app,
        session,
        state,
    };
    c.batch_unchanged("edit group open", "owner");
    c
}
fn saved(c: &mut ReorderBoard, step: &str, name: &str, colour: &str) {
    c.app.act(step);
    c.state = c.state.edited_group(&c.app.f, "g-z", name, colour);
    c.access_checkpoint("owner");
}
pub fn edit() {
    let mut c = open();
    for step in ["edit group cancel", "edit group rename"] {
        c.batch_unchanged(step, "owner");
    }
    let mut db = c.app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.query_one(
        "SELECT revision FROM boards WHERE id=$1 FOR UPDATE",
        &[&SELECTED],
    )
    .unwrap();
    c.batch_unchanged("edit group pending", "owner");
    lock.rollback().unwrap();
    saved(&mut c, "edit group committed", "Work", STORED);
    saved(&mut c, "edit group name only", "Ink", STORED);
    saved(&mut c, "edit group colour only", "Ink", BLUE);
    c.batch_unchanged("edit group reload blue", "owner");
}
pub fn review() {
    let mut c = open();
    saved(&mut c, "edit group unchanged", "Work", STORED);
    c.batch_unchanged("edit group draft", "owner");
    let mut db = c.app.f.connect();
    db.batch_execute(FAULT).unwrap();
    c.batch_unchanged("edit group refusal", "owner");
    db.batch_execute("DROP TRIGGER refuse_edit ON board_groups")
        .unwrap();
    let board = BoardId::new(SELECTED).unwrap();
    let mut tx = db.transaction().unwrap();
    store::edit_group(&mut tx, board, "g-z", 8, 2, "Peer", "#abcdef").unwrap();
    tx.commit().unwrap();
    c.state = c.state.edited_group(&c.app.f, "g-z", "Peer", "#abcdef");
    for step in [
        "edit group conflict",
        "edit group review",
        "edit group adopt",
    ] {
        c.batch_unchanged(step, "owner");
    }
    saved(&mut c, "edit group refresh fault", "Reserve", GREEN);
    c.batch_unchanged("edit group read accepted", "owner");
    c.batch_unchanged("edit group reload green", "owner");
    saved(&mut c, "edit group name green", "Ink", GREEN);
    saved(&mut c, "edit group colour blue", "Ink", BLUE);
    c.batch_unchanged("edit group open ink", "owner");
    let mut tx = db.transaction().unwrap();
    store::transition_group(&mut tx, board, "g-z", store::Lifecycle::Archive, 12).unwrap();
    tx.commit().unwrap();
    c.state = c.state.group_transition(&c.app.f, "g-z", false);
    c.batch_unchanged("edit group archived conflict", "owner");
    c.batch_unchanged("edit group archived review", "owner");
}
pub fn demote() {
    let mut c = open();
    c.batch_unchanged("edit group rename", "owner");
    check_demotion(c, "edit group");
}
pub(super) fn check_demotion(mut c: ReorderBoard, action: &str) {
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
            c.batch_unchanged(&format!("{action} denied"), role);
        }
        c.batch_unchanged(&format!("{action} {role}"), role);
    }
    c.app.catalog.membership(&c.app.f, false);
    c.app.act("new group withdrawn");
    assert_eq!(c.state, Snapshot::read(&c.app.f));
    c.app.catalog.check_grants(&c.app.f, false);
    assert!(c.session.present(&c.app.f));
    c.session.revoke(&c.app.f);
    c.app.act("new group expired");
    assert_eq!(c.state, Snapshot::read(&c.app.f));
    c.app.catalog.check_grants(&c.app.f, false);
    assert!(!c.session.present(&c.app.f));
}
