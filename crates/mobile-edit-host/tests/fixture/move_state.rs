//! Independent full-row oracle (including inactive, sibling and direct rows).
use crate::{
    fixture::Fixture,
    selection::{self, SELECTED},
};
use serde_json::{Value, json};
use uuid::Uuid;

fn snapshot(f: &Fixture, sql: &str) -> Vec<Value> {
    f.connect()
        .query(sql, &[])
        .unwrap()
        .into_iter()
        .map(|r| serde_json::from_str(r.get(0)).unwrap())
        .collect()
}

const ITEMS: &str = "SELECT row_to_json(i)::text FROM board_items i ORDER BY id";
const GROUPS: &str = "SELECT row_to_json(g)::text FROM board_groups g ORDER BY board_id,id";

pub struct MoveState {
    items: Vec<Value>,
    groups: Vec<Value>,
}
impl MoveState {
    pub fn seed(f: &Fixture, creator: Uuid) -> Self {
        selection::check_groups(f, false);
        let mut db = f.connect();
        for (id, time) in [(61, "2001-01-01T00:00:00Z"), (59, "2001-01-02T00:00:00Z")] {
            assert_eq!(db.execute("UPDATE board_items SET position=2,created_at=$2::text::timestamptz,created_by=$3,creation_title=title,creation_group_id=group_id WHERE id=$1 AND board_id=$4", &[&Uuid::from_u128(id),&time,&creator,&SELECTED]).unwrap(),1);
        }
        Self {
            items: snapshot(f, ITEMS),
            groups: snapshot(f, GROUPS),
        }
    }
    pub fn check(&self, f: &Fixture, group: &str, title: &str, revision: i64, stamp: i64) {
        let mut expected = self.items.clone();
        let row = expected
            .iter_mut()
            .find(|r| r["id"] == json!(Uuid::from_u128(61)))
            .unwrap();
        row["group_id"] = json!(group);
        row["title"] = json!(title);
        row["revision"] = json!(revision);
        row["group_revision"] = json!(stamp);
        assert_eq!(snapshot(f, ITEMS), expected);
    }
    pub fn groups(&self, f: &Fixture, archived: bool) {
        let mut expected = self.groups.clone();
        if archived {
            expected
                .iter_mut()
                .find(|g| g["board_id"] == json!(SELECTED) && g["id"] == "g-20")
                .unwrap()["active"] = json!(false);
        }
        assert_eq!(snapshot(f, GROUPS), expected);
    }
}
