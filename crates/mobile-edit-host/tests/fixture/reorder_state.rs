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
    views: Vec<Value>,
    folds: Vec<Value>,
}
impl Snapshot {
    pub fn folded(&self, f: &Fixture, revision: i64, ids: &[&str]) -> Self {
        let mut expected = self.clone();
        let b = expected
            .boards
            .iter_mut()
            .find(|b| b["id"] == json!(SELECTED))
            .unwrap();
        b["revision"] = json!(b["revision"].as_i64().unwrap() + 1);
        expected.views = vec![json!({"board_id":SELECTED,"revision":revision})];
        let mut ids = ids.to_vec();
        ids.sort_unstable();
        expected.folds = ids
            .into_iter()
            .map(|id| json!({"board_id":SELECTED,"group_id":id}))
            .collect();
        assert_eq!(Self::read(f), expected);
        expected
    }
    pub fn reordered_groups(&self, f: &Fixture, ids: &[&str]) -> Self {
        let mut expected = self.clone();
        let board = expected
            .boards
            .iter_mut()
            .find(|b| b["id"] == json!(SELECTED))
            .unwrap();
        let revision = board["revision"].as_i64().unwrap() + 1;
        board["revision"] = json!(revision);
        let board_id = BoardId::new(SELECTED).unwrap();
        let (read_revision, groups) = store::load_group_snapshot(&mut f.connect(), board_id)
            .ok()
            .unwrap();
        assert_eq!(read_revision, revision);
        assert_eq!(groups.len(), ids.len());
        let mut flat = Vec::new();
        for (position, (&id, group)) in ids.iter().zip(groups).enumerate() {
            let row = expected
                .groups
                .iter_mut()
                .find(|r| r["board_id"] == json!(SELECTED) && r["id"] == id)
                .unwrap();
            row["position"] = json!(position);
            row["revision"] = json!(row["revision"].as_i64().unwrap() + 1);
            assert_eq!(group.id, id);
            assert_eq!(json!(group.revision), row["revision"]);
            let children: &[u128] = match id {
                "g-z" => &[61, 63],
                "g-old" => &[59],
                "g-end" => &[],
                _ => panic!("unexpected group"),
            };
            assert_eq!(
                group
                    .items
                    .iter()
                    .map(|r| r.id.as_uuid().as_u128())
                    .collect::<Vec<_>>(),
                children
            );
            flat.extend_from_slice(children);
        }
        assert_eq!(
            store::load_items(&mut f.connect(), board_id)
                .unwrap()
                .unwrap()
                .iter()
                .map(|r| r.id.as_uuid().as_u128())
                .collect::<Vec<_>>(),
            flat
        );
        assert_eq!(Self::read(f), expected);
        expected
    }
    pub fn edited_group(&self, f: &Fixture, id: &str, name: &str, colour: &str) -> Self {
        let mut expected = self.clone();
        let board = expected
            .boards
            .iter_mut()
            .find(|r| r["id"] == json!(SELECTED))
            .unwrap();
        let group = expected
            .groups
            .iter_mut()
            .find(|r| r["board_id"] == json!(SELECTED) && r["id"] == id)
            .unwrap();
        group["name"] = json!(name);
        group["colour"] = json!(colour);
        for row in [group, board] {
            row["revision"] = json!(row["revision"].as_i64().unwrap() + 1);
        }
        assert_eq!(Self::read(f), expected);
        expected
    }
    pub fn created_group(
        &self,
        f: &Fixture,
        id: &str,
        name: &str,
        colour: &str,
        position: i64,
    ) -> Self {
        let mut expected = self.clone();
        let board = expected
            .boards
            .iter_mut()
            .find(|b| b["id"] == json!(SELECTED))
            .unwrap();
        board["revision"] = json!(board["revision"].as_i64().unwrap() + 1);
        expected.groups.push(json!({"id":id,"board_id":SELECTED,"name":name,"position":position,"active":true,"deleted":false,"revision":1,"colour":colour}));
        expected.groups.sort_by(|a, b| {
            (a["board_id"].as_str(), a["id"].as_str())
                .cmp(&(b["board_id"].as_str(), b["id"].as_str()))
        });
        assert_eq!(Self::read(f), expected);
        expected
    }
    pub fn group_transition(&self, f: &Fixture, id: &str, active: bool) -> Self {
        let mut expected = self.clone();
        let board = expected
            .boards
            .iter_mut()
            .find(|r| r["id"] == json!(SELECTED))
            .unwrap();
        board["revision"] = json!(board["revision"].as_i64().unwrap() + 1);
        let group = expected
            .groups
            .iter_mut()
            .find(|r| r["board_id"] == json!(SELECTED) && r["id"] == id)
            .unwrap();
        assert_eq!(group["active"], !active);
        group["active"] = json!(active);
        group["revision"] = json!(group["revision"].as_i64().unwrap() + 1);
        assert_eq!(Self::read(f), expected);
        expected
    }
    pub fn lifecycle(&self, f: &Fixture, actor: Uuid, action: &str) -> Self {
        let now = Self::read(f);
        let mut expected = self.clone();
        let id = Uuid::from_u128(67);
        let item = expected
            .items
            .iter_mut()
            .find(|i| i["id"] == json!(id))
            .unwrap();
        let board = expected
            .boards
            .iter_mut()
            .find(|i| i["id"] == json!(SELECTED))
            .unwrap();
        let b = board["revision"].as_i64().unwrap();
        let i = item["revision"].as_i64().unwrap();
        let archived = action == "archive";
        item["state"] = json!(i32::from(archived));
        item["revision"] = json!(i + 1);
        board["revision"] = json!(b + 1);
        let op = &now.receipts.last().unwrap()["operation_id"];
        assert!(!Uuid::parse_str(op.as_str().unwrap()).unwrap().is_nil());
        expected.receipts.push(json!({"operation_id":op,"item_id":id,"actor_id":actor,"board_id":SELECTED,"board_revision":b,"item_revision":i,"lifecycle":action}));
        assert_eq!(now, expected);
        let id = BoardId::new(SELECTED).unwrap();
        let groups = store::load_groups(&mut f.connect(), id).ok().unwrap();
        let ids = |items: Vec<store::StoredItem>| {
            items
                .into_iter()
                .map(|i| i.id.as_uuid().as_u128())
                .collect::<Vec<_>>()
        };
        let first = if archived {
            vec![73, 61, 77]
        } else {
            vec![73, 61, 67, 77]
        };
        assert_eq!(
            groups.into_iter().map(|g| ids(g.items)).collect::<Vec<_>>(),
            [first.clone(), vec![], vec![59]]
        );
        assert_eq!(
            ids(store::load_items(&mut f.connect(), id).unwrap().unwrap()),
            [first, vec![59]].concat()
        );
        now
    }
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
            views: rows("SELECT row_to_json(v)::text FROM main_tables v ORDER BY board_id"),
            folds: rows(
                "SELECT row_to_json(f)::text FROM main_table_folds f ORDER BY board_id,group_id",
            ),
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
