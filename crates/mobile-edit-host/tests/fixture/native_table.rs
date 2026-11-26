//! Packet 43's duplicate-Work data, shared by the native HTTP and Windows checks.
use crate::{
    fixture::Fixture,
    group_edit,
    selection::{self, SELECTED, Selection},
};
use boardcraft_mobile_edit_client::EditClient;
use serde_json::{Value, json};
use uuid::Uuid;

pub const A: &str = "g-old";
pub const B: &str = "g-z";
pub const C: &str = "g-end";

pub struct NativeTable {
    pub selection: Selection,
    pub f: Fixture,
    pub member: Uuid,
}

impl NativeTable {
    pub fn new() -> Self {
        let f = Fixture::start();
        let selection = Selection::new(&f);
        selection::seed_groups(&f);
        group_edit::seed(&f);
        let member = f
            .connect()
            .query_one("SELECT id FROM accounts WHERE username='reader'", &[])
            .unwrap()
            .get(0);
        Self {
            selection,
            f,
            member,
        }
    }
    pub fn client(&self) -> EditClient {
        EditClient::with_bearer(self.selection.host.port, self.selection.token.encoded()).unwrap()
    }
    pub fn role(&self, role: &str) {
        self.f
            .connect()
            .execute(
                "UPDATE accounts SET role=$2 WHERE id=$1",
                &[&self.member, &role],
            )
            .unwrap();
    }
    pub fn state(&self) -> Value {
        let mut db = self.f.connect();
        let mut result = json!({});
        for (key, sql) in [
            (
                "boards",
                "SELECT row_to_json(r)::text FROM boards r ORDER BY id",
            ),
            (
                "groups",
                "SELECT row_to_json(r)::text FROM board_groups r ORDER BY board_id,id",
            ),
            (
                "items",
                "SELECT row_to_json(r)::text FROM board_items r ORDER BY id",
            ),
            (
                "views",
                "SELECT row_to_json(r)::text FROM main_tables r ORDER BY board_id",
            ),
            (
                "folds",
                "SELECT row_to_json(r)::text FROM main_table_folds r ORDER BY board_id,group_id",
            ),
            (
                "receipts",
                "SELECT row_to_json(r)::text FROM group_reorder_receipts r ORDER BY board_revision",
            ),
        ] {
            result[key] = Value::Array(
                db.query(sql, &[])
                    .unwrap()
                    .iter()
                    .map(|r| serde_json::from_str(r.get(0)).unwrap())
                    .collect(),
            );
        }
        result
    }
    pub fn folded(&self, before: &Value, revision: i64, ids: &[&str]) -> Value {
        let mut expected = before.clone();
        let board = expected["boards"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["id"] == json!(SELECTED))
            .unwrap();
        board["revision"] = json!(board["revision"].as_i64().unwrap() + 1);
        expected["views"] = json!([{"board_id":SELECTED,"revision":revision}]);
        let mut ids = ids.to_vec();
        ids.sort_unstable();
        expected["folds"] = json!(
            ids.iter()
                .map(|id| json!({"board_id":SELECTED,"group_id":id}))
                .collect::<Vec<_>>()
        );
        assert_eq!(self.state(), expected);
        expected
    }
    pub fn check_read(&self, revision: i64, folded: &[&str], editable: bool) {
        let client = self.client();
        let read = client.read_main_table(SELECTED).unwrap();
        assert_eq!(read.view_revision(), revision);
        assert_eq!(read.is_editable(), editable);
        assert_eq!(
            read.groups()
                .iter()
                .map(|g| (g.id(), g.name(), g.items().len(), g.is_collapsed()))
                .collect::<Vec<_>>(),
            [
                (A, "Work", 1, folded.contains(&A)),
                (B, "Work", 2, folded.contains(&B)),
                (C, "Reserve", 0, folded.contains(&C))
            ]
        );
        let roots: Vec<_> = read
            .groups()
            .iter()
            .flat_map(|g| g.items())
            .cloned()
            .collect();
        assert_eq!(
            roots
                .iter()
                .map(|r| (r.id.as_u128(), r.title.as_str(), r.revision))
                .collect::<Vec<_>>(),
            [(59, "Ink", 1), (61, "Ink", 1), (63, "Ink", 1)]
        );
        assert_eq!(client.read_selected_board(SELECTED).unwrap(), roots);
        self.f.check_board(SELECTED, &roots);
    }
}
