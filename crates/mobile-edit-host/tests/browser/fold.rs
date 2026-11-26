use crate::{ReorderBoard, group_edit_browser::open, reorder_state::Snapshot, selection::SELECTED};
use boardcraft_account::Role;
use boardcraft_board::{BoardId, store};
use uuid::Uuid;

const A: &str = "g-old";
const B: &str = "g-z";
const C: &str = "g-end";
fn save(c: &mut ReorderBoard, step: &str, rev: i64, folds: &[&str], role: &str) {
    c.app.act(step);
    c.state = c.state.folded(&c.app.f, rev, folds);
    c.access_checkpoint(role);
}
fn role(c: &ReorderBoard, role: &str) {
    c.app
        .f
        .connect()
        .execute(
            "UPDATE accounts SET role=$2 WHERE id=$1",
            &[&c.app.account, &role],
        )
        .unwrap();
}
fn reorder(c: &mut ReorderBoard, step: &str, ids: &[&str]) {
    c.app.act(step);
    c.state = c.state.reordered_groups(&c.app.f, ids);
    c.access_checkpoint("owner");
}
fn folded_selection() -> ReorderBoard {
    let mut c = open();
    save(&mut c, "fold space", 2, &[B], "owner");
    c.batch_unchanged("fold range", "owner");
    save(&mut c, "fold expand", 3, &[], "owner");
    save(&mut c, "fold collapse", 4, &[B], "owner");
    c.batch_unchanged("fold select hidden", "owner");
    c.batch_unchanged("fold reload B", "owner");
    c
}
pub fn single() {
    folded_selection();
}
pub fn all() {
    let mut c = folded_selection();
    reorder(&mut c, "fold move B", &[B, A, C]);
    role(&c, "member");
    c.batch_unchanged("fold member", "member");
    save(&mut c, "fold all", 5, &[B, A, C], "member");
    save(&mut c, "fold none", 6, &[], "member");
    save(&mut c, "fold empty", 7, &[C], "member");
    let mut c = open();
    save(&mut c, "fold start B", 2, &[B], "owner");
    reorder(&mut c, "fold drag B", &[B, A, C]);
    reorder(&mut c, "fold drag C", &[C, B, A]);
    save(&mut c, "fold after drag", 3, &[], "owner");
}
pub fn viewer() {
    let mut c = open();
    save(&mut c, "fold start C", 2, &[C], "owner");
    let password = Uuid::new_v4().to_string();
    let guest = c.app.f.seed_account("guest", &password, Role::Viewer);
    let mut db = c.app.f.connect();
    db.execute("INSERT INTO workspace_members SELECT workspace_id,$2 FROM workspace_members WHERE account_id=$1",&[&c.app.account,&guest]).unwrap();
    db.execute("INSERT INTO board_subscribers SELECT board_id,$2 FROM board_subscribers WHERE account_id=$1",&[&c.app.account,&guest]).unwrap();
    let _guest_profile = c.app.viewer_profile(&password);
    assert_eq!(c.state, Snapshot::read(&c.app.f));
    assert!(c.session.present(&c.app.f));
}
pub fn recover() {
    let mut c = open();
    let mut db = c.app.f.connect();
    db.batch_execute("CREATE FUNCTION refuse_fold() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER refuse_fold AFTER INSERT ON main_tables DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION refuse_fold()").unwrap();
    let mut lock = db.transaction().unwrap();
    lock.query_one(
        "SELECT revision FROM boards WHERE id=$1 FOR UPDATE",
        &[&SELECTED],
    )
    .unwrap();
    c.batch_unchanged("fold pending", "owner");
    lock.rollback().unwrap();
    c.batch_unchanged("fold refused", "owner");
    db.batch_execute("DROP TRIGGER refuse_fold ON main_tables")
        .unwrap();
    c.batch_unchanged("fold read baseline", "owner");
    let mut peer = db.transaction().unwrap();
    store::set_main_table_fold(
        &mut peer,
        BoardId::new(SELECTED).unwrap(),
        1,
        &[C.to_owned()],
        true,
    )
    .unwrap();
    peer.commit().unwrap();
    c.state = c.state.folded(&c.app.f, 2, &[C]);
    c.batch_unchanged("fold conflict", "owner");
    c.batch_unchanged("fold read peer", "owner");
    save(&mut c, "fold fresh", 3, &[B, C], "owner");
    c.batch_unchanged("fold read commit", "owner");
    save(&mut c, "fold unconfirmed", 4, &[B], "owner");
    c.batch_unchanged("fold read unconfirmed", "owner");
    role(&c, "member");
    c.batch_unchanged("fold render member", "member");
    role(&c, "viewer");
    c.batch_unchanged("fold denied", "viewer");
    role(&c, "member");
    c.batch_unchanged("fold render member", "member");
    c.session.revoke(&c.app.f);
    c.app.act("fold expired");
    assert_eq!(c.state, Snapshot::read(&c.app.f));
    assert!(!c.session.present(&c.app.f));
}
