use crate::{
    ReorderBoard,
    group_edit::FAULT,
    group_edit_browser::{check_demotion, open},
    selection::SELECTED,
};
use boardcraft_board::{BoardId, store};

const ABC: [&str; 3] = ["g-old", "g-z", "g-end"];
const BAC: [&str; 3] = ["g-z", "g-old", "g-end"];
const ACB: [&str; 3] = ["g-old", "g-end", "g-z"];
const CAB: [&str; 3] = ["g-end", "g-old", "g-z"];
const BCA: [&str; 3] = ["g-z", "g-end", "g-old"];

fn saved(c: &mut ReorderBoard, step: &str, order: &[&str]) {
    c.app.act(step);
    c.state = c.state.reordered_groups(&c.app.f, order);
    c.access_checkpoint("owner");
}
pub fn moves() {
    let mut c = open();
    c.batch_unchanged("order groups boundaries", "owner");
    let mut db = c.app.f.connect();
    let mut tx = db.transaction().unwrap();
    tx.query_one(
        "SELECT revision FROM boards WHERE id=$1 FOR UPDATE",
        &[&SELECTED],
    )
    .unwrap();
    c.batch_unchanged("order groups pending", "owner");
    tx.rollback().unwrap();
    saved(&mut c, "order groups committed", &BAC);
    saved(&mut c, "order groups down", &ABC);
    saved(&mut c, "order groups past empty", &ACB);
    c.batch_unchanged("order groups document reload", "owner");
    c.batch_unchanged("order groups sole", "owner");
}
pub fn recovery() {
    let mut c = open();
    saved(&mut c, "order groups up", &BAC);
    let mut db = c.app.f.connect();
    db.batch_execute(FAULT).unwrap();
    c.batch_unchanged("order groups refusal", "owner");
    db.batch_execute("DROP TRIGGER refuse_edit ON board_groups")
        .unwrap();
    saved(&mut c, "order groups down", &ABC);
    c.batch_unchanged("order groups open", "owner");
    let mut tx = db.transaction().unwrap();
    store::reorder_groups(
        &mut tx,
        BoardId::new(SELECTED).unwrap(),
        9,
        &ACB.map(String::from),
    )
    .unwrap();
    tx.commit().unwrap();
    c.state = c.state.reordered_groups(&c.app.f, &ACB);
    c.batch_unchanged("order groups conflict", "owner");
    c.batch_unchanged("order groups read peer", "owner");
    saved(&mut c, "order groups new neighbour", &ABC);
    c.batch_unchanged("order groups read commit", "owner");
}
pub fn demote() {
    let mut c = open();
    c.batch_unchanged("order groups open", "owner");
    check_demotion(c, "order groups");
}

pub fn across() {
    let mut c = open();
    c.batch_unchanged("drag empty target", "owner");
    let mut db = c.app.f.connect();
    let mut lock = db.transaction().unwrap();
    lock.query_one(
        "SELECT revision FROM boards WHERE id=$1 FOR UPDATE",
        &[&SELECTED],
    )
    .unwrap();
    c.batch_unchanged("drag pending", "owner");
    lock.rollback().unwrap();
    saved(&mut c, "drag committed", &CAB);
    saved(&mut c, "drag child", &ABC);
    c.batch_unchanged("drag reopen", "owner");
}

pub fn cancel_drag() {
    let mut c = open();
    c.batch_unchanged("drag cancel", "owner");
    c.batch_unchanged("drag wrong surfaces", "owner");
}

pub fn scroll() {
    let mut c = open();
    saved(&mut c, "drag scroll", &BCA);
    c.batch_unchanged("drag stop scrolling", "owner");
    c.batch_unchanged("drag leave", "owner");
}

pub fn recover_drag() {
    let mut c = open();
    let mut db = c.app.f.connect();
    db.batch_execute(FAULT).unwrap();
    c.batch_unchanged("drag refused", "owner");
    db.batch_execute("DROP TRIGGER refuse_edit ON board_groups")
        .unwrap();
    c.batch_unchanged("drag hold", "owner");
    let mut tx = db.transaction().unwrap();
    store::reorder_groups(
        &mut tx,
        BoardId::new(SELECTED).unwrap(),
        7,
        &BAC.map(String::from),
    )
    .unwrap();
    tx.commit().unwrap();
    c.state = c.state.reordered_groups(&c.app.f, &BAC);
    c.batch_unchanged("drag stale", "owner");
    c.batch_unchanged("drag read peer", "owner");
    saved(&mut c, "drag fresh", &BCA);
    c.batch_unchanged("drag read commit", "owner");
    // Access changes use a fresh /7 fixture, with the pointer already held.
    let mut c = open();
    c.batch_unchanged("drag hold", "owner");
    check_demotion(c, "drag");
}
