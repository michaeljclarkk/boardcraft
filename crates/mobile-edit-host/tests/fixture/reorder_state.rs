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

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    items: Vec<Value>,
    boards: Vec<Value>,
    groups: Vec<Value>,
    receipts: Vec<Value>,
}
impl Snapshot {
    // Build the expected committed records from the pre-attempt snapshot, then
    // verify the two public read shapes independently of the POST acknowledgment.
    pub fn batch(
        &self,
        f: &Fixture,
        actor: Uuid,
        group: &str,
        destination: &[u128],
        selected: &[u128],
    ) -> Self {
        let now = Self::read(f);
        let mut expected = self.clone();
        let id = |n| json!(Uuid::from_u128(n));
        let base = expected
            .boards
            .iter_mut()
            .find(|r| r["id"] == json!(SELECTED))
            .unwrap();
        let rev = base["revision"].as_i64().unwrap();
        base["revision"] = json!(rev + 1);
        let revisions: Vec<_> = selected
            .iter()
            .map(|n| self.items.iter().find(|r| r["id"] == id(*n)).unwrap()["revision"].clone())
            .collect();
        for (pos, n) in destination.iter().enumerate() {
            let row = expected
                .items
                .iter_mut()
                .find(|r| r["id"] == id(*n))
                .unwrap();
            row["position"] = json!(pos);
            if selected.contains(n) {
                row["group_id"] = json!(group);
                let version = row["revision"].as_i64().unwrap() + 1;
                for field in ["revision", "position_revision", "group_revision"] {
                    row[field] = json!(version);
                }
            }
        }
        let receipt = now.receipts.last().unwrap();
        let op = Uuid::parse_str(receipt["operation_id"].as_str().unwrap()).unwrap();
        assert!(!op.is_nil());
        let mut affected: Vec<Uuid> =
            serde_json::from_value(receipt["affected_ids"].clone()).unwrap();
        affected.sort();
        let ids: Vec<_> = selected.iter().copied().map(Uuid::from_u128).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(affected, sorted);
        expected.receipts.push(json!({"operation_id":op,"actor_id":actor,"board_id":SELECTED,"board_revision":rev,"group_id":group,"selected_ids":ids,"selected_revisions":revisions,"affected_ids":receipt["affected_ids"]}));
        assert_eq!(now, expected);
        let board = BoardId::new(SELECTED).unwrap();
        let (_, groups) = store::load_group_snapshot(&mut f.connect(), board)
            .ok()
            .unwrap();
        assert_eq!(
            groups
                .iter()
                .find(|g| g.id == group)
                .unwrap()
                .items
                .iter()
                .map(|r| r.id.as_uuid().as_u128())
                .collect::<Vec<_>>(),
            destination
        );
        let flat: Vec<_> = store::load_items(&mut f.connect(), board)
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(
            flat,
            groups
                .into_iter()
                .flat_map(|g| g.items.into_iter().map(|r| r.id))
                .collect::<Vec<_>>()
        );
        now
    }
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
                "SELECT jsonb_strip_nulls(to_jsonb(r))::text FROM group_reorder_receipts r ORDER BY board_revision",
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
