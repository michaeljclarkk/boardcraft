use crate::{
    fixture::Fixture,
    group_create,
    selection::{SELECTED, SIBLING},
};
use uuid::Uuid;

pub const STORED: &str = "#443322";
pub const FAULT: &str = "CREATE FUNCTION refuse_edit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER refuse_edit AFTER UPDATE ON board_groups DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION refuse_edit()";

pub fn seed(f: &Fixture) {
    group_create::seed(f);
    let mut db = f.connect();
    let mut tx = db.transaction().unwrap();
    tx.execute("UPDATE boards SET revision=7 WHERE id=$1", &[&SELECTED])
        .unwrap();
    tx.execute(
        "UPDATE board_groups SET colour=$3 WHERE board_id IN ($1,$2)",
        &[&SELECTED, &SIBLING, &STORED],
    )
    .unwrap();
    tx.execute(
        "UPDATE board_groups SET name='Work',position=3 WHERE board_id=$1 AND id='g-old'",
        &[&SELECTED],
    )
    .unwrap();
    for (id, state) in [(63, 0i32), (67, 1)] {
        tx.execute("INSERT INTO board_items(id,board_id,group_id,title,position,state) VALUES ($1,$2,'g-z','Ink',8,$3)", &[&Uuid::from_u128(id),&SELECTED,&state]).unwrap();
    }
    tx.commit().unwrap();
}
