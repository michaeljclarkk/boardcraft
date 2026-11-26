use crate::{
    fixture::Fixture,
    selection::{SELECTED, SIBLING},
};
use uuid::Uuid;

pub const BLUE: &str = "#579bfc";
pub const GREEN: &str = "#00c875";
pub const FAULT: &str = "CREATE FUNCTION refuse_group() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER refuse_group AFTER INSERT ON board_groups DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION refuse_group()";

pub fn seed(f: &Fixture) {
    let mut db = f.connect();
    let mut tx = db.transaction().unwrap();
    tx.execute(
        "DELETE FROM board_items WHERE id=$1",
        &[&Uuid::from_u128(73)],
    )
    .unwrap();
    for (board, old, new, name, pos, active) in [
        (SELECTED, "g-20", "g-old", "New group", 0i64, true),
        (SELECTED, "g-90", "g-z", "Work", 3, true),
        (SELECTED, "g-10", "g-end", "Reserve", 12, true),
        (SELECTED, "g-30", "g-hidden", "Retired", 20, false),
        (SIBLING, "g-70", "g-peer", "Work", 1, true),
    ] {
        tx.execute(
            "INSERT INTO board_groups(board_id,id,name,position,active) VALUES ($1,$2,$3,$4,$5)",
            &[&board, &new, &name, &pos, &active],
        )
        .unwrap();
        tx.execute(
            "UPDATE board_items SET group_id=$3,title='Ink' WHERE board_id=$1 AND group_id=$2",
            &[&board, &old, &new],
        )
        .unwrap();
        tx.execute(
            "DELETE FROM board_groups WHERE board_id=$1 AND id=$2",
            &[&board, &old],
        )
        .unwrap();
    }
    tx.commit().unwrap();
}
