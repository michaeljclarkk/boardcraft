use crate::{
    fixture::Fixture,
    selection::{self, SELECTED},
};
use boardcraft_board::{BoardId, store};
use serde_json::{Value, json};
use uuid::Uuid;

pub fn seed(f: &Fixture, actor: Uuid) {
    selection::check_groups(f, false);
    let mut db = f.connect();
    db.execute(
        "UPDATE board_items SET position=10 WHERE id=$1",
        &[&Uuid::from_u128(61)],
    )
    .unwrap();
    db.execute(
        "UPDATE board_items SET position=2 WHERE id=$1",
        &[&Uuid::from_u128(59)],
    )
    .unwrap();
    for (n, pos, title) in [(67, 30i64, "Ink"), (77, 40, "Fold")] {
        db.execute("INSERT INTO board_items(id,board_id,group_id,title,position,created_by,creation_group_id,creation_title) VALUES ($1,$2,'g-90',$3,$4,$5,'g-90',$3)", &[&Uuid::from_u128(n),&SELECTED,&title,&pos,&actor]).unwrap();
    }
}

#[derive(Debug, PartialEq)]
pub struct Snapshot {
    items: Vec<Value>,
    boards: Vec<Value>,
    groups: Vec<Value>,
    receipts: Vec<Value>,
}
impl Snapshot {
    pub fn read(f: &Fixture) -> Self {
        let mut db = f.connect();
        let mut rows = |sql| {
            db.query(sql, &[])
                .unwrap()
                .into_iter()
                .map(|r| serde_json::from_str(r.get(0)).unwrap())
                .collect()
        };
        Self {
            items: rows("SELECT row_to_json(i)::text FROM board_items i ORDER BY id"),
            boards: rows("SELECT row_to_json(b)::text FROM boards b ORDER BY id"),
            groups: rows("SELECT row_to_json(g)::text FROM board_groups g ORDER BY board_id,id"),
            receipts: rows(
                "SELECT row_to_json(r)::text FROM group_reorder_receipts r ORDER BY board_revision",
            ),
        }
    }

    pub fn check(&self, f: &Fixture, actor: Uuid, drops: usize) {
        let now = Self::read(f);
        let rev = 1 + drops;
        let order = if drops == 1 {
            [73, 67, 61, 77]
        } else {
            [73, 61, 67, 77]
        };
        let mut items = self.items.clone();
        let mut boards = self.boards.clone();
        boards
            .iter_mut()
            .find(|r| r["id"] == json!(SELECTED))
            .unwrap()["revision"] = json!(rev);
        if drops > 0 {
            for (pos, n) in order.iter().enumerate() {
                let r = items
                    .iter_mut()
                    .find(|r| r["id"] == json!(Uuid::from_u128(*n)))
                    .unwrap();
                r["position"] = json!(pos);
                if *n == 67 {
                    for field in ["revision", "group_revision", "position_revision"] {
                        r[field] = json!(rev);
                    }
                }
            }
        }
        assert_eq!(now.items, items);
        assert_eq!(now.boards, boards);
        assert_eq!(now.groups, self.groups);
        assert_eq!(now.receipts.len(), drops);
        for (i, r) in now.receipts.iter().enumerate() {
            let op = Uuid::parse_str(r["operation_id"].as_str().unwrap()).unwrap();
            assert!(!op.is_nil());
            assert_eq!(
                *r,
                json!({"operation_id":op,"actor_id":actor,"board_id":SELECTED,"group_id":"g-90","item_id":Uuid::from_u128(67),"anchor_id":Uuid::from_u128(61),"after_anchor":i==1,"item_revision":i+1,"board_revision":i+1})
            );
        }
        let board = BoardId::new(SELECTED).unwrap();
        let expected: Vec<_> = order.into_iter().chain([59]).collect();
        let ids = |rows: Vec<store::StoredItem>| {
            rows.into_iter()
                .map(|r| r.id.as_uuid().as_u128())
                .collect::<Vec<_>>()
        };
        let (version, groups) = store::load_group_snapshot(&mut f.connect(), board)
            .ok()
            .unwrap();
        assert_eq!(version, rev as i64);
        assert_eq!(
            ids(groups.into_iter().flat_map(|g| g.items).collect()),
            expected
        );
        assert_eq!(
            ids(store::load_items(&mut f.connect(), board).unwrap().unwrap()),
            expected
        );
    }
}
