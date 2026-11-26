//! Reuse packet 36's seven-row fixture and independent whole-store oracle.
use crate::{
    fixture::Fixture,
    reorder_state::{self, Snapshot},
    selection::{self, Selection},
};
use boardcraft_mobile_edit_client::EditClient;
use uuid::Uuid;

pub struct NativeSelection {
    pub selection: Selection,
    pub f: Fixture,
    pub member: Uuid,
}
impl NativeSelection {
    pub fn new() -> Self {
        let f = Fixture::start();
        let selection = Selection::new(&f);
        selection::seed_groups(&f);
        let member = f
            .connect()
            .query_one("SELECT id FROM accounts WHERE username='reader'", &[])
            .unwrap()
            .get(0);
        reorder_state::seed(&f, member);
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
    pub fn state(&self) -> Snapshot {
        Snapshot::read(&self.f)
    }
    pub fn check_access(&self, role: &str, joined: bool, live: bool) {
        let mut db = self.f.connect();
        assert_eq!(
            db.query_one("SELECT role FROM accounts WHERE id=$1", &[&self.member])
                .unwrap()
                .get::<_, String>(0),
            role
        );
        assert_eq!(
            db.query_one(
                "SELECT count(*) FROM workspace_members WHERE account_id=$1",
                &[&self.member]
            )
            .unwrap()
            .get::<_, i64>(0),
            i64::from(joined)
        );
        let boards: Vec<Uuid> = db
            .query(
                "SELECT board_id FROM board_subscribers WHERE account_id=$1 ORDER BY board_id",
                &[&self.member],
            )
            .unwrap()
            .iter()
            .map(|r| r.get(0))
            .collect();
        assert_eq!(
            boards,
            [selection::SELECTED, selection::SIBLING, selection::QUARTZ]
        );
        assert_eq!(self.selection.session.present(&self.f), live);
    }
}
