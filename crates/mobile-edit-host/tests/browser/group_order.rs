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
