//! Shared database oracle for the HTTP and graphical destination checks.
use crate::fixture::Fixture;
#[cfg(test)]
use crate::stored_session::StoredSession;
use boardcraft_account::Role;
#[cfg(test)]
use boardcraft_account::Token;
use boardcraft_board::{Board, BoardId, ItemId, store};
use boardcraft_mobile_edit_client::SavedItem;
#[cfg(test)]
use boardcraft_mobile_edit_host::Host;
use uuid::Uuid;

pub const QUARTZ: Uuid = Uuid::from_u128(203);
pub const SIBLING: Uuid = Uuid::from_u128(202);
pub const SELECTED: Uuid = Uuid::from_u128(201);
pub const HIDDEN: Uuid = Uuid::from_u128(204);

#[cfg(test)]
pub fn seed_groups(f: &Fixture) {
    let mut db = f.connect();
    let mut tx = db.transaction().unwrap();
    for (board, id, name, pos, active) in [
        (SELECTED, "g-20", "Work", 2i64, true),
        (SIBLING, "g-70", "Work", 1, true),
        (SELECTED, "g-10", "Reserve", 2, true),
        (SELECTED, "g-30", "Retired", 0, false),
        (SELECTED, "g-90", "Work", 1, true),
    ] {
        tx.execute(
            "INSERT INTO board_groups(board_id,id,name,position,active) VALUES ($1,$2,$3,$4,$5)",
            &[&board, &id, &name, &pos, &active],
        )
        .unwrap();
    }
    tx.execute(
        "UPDATE board_items SET group_id='g-90',position=position*2 WHERE board_id=$1",
        &[&SELECTED],
    )
    .unwrap();
    tx.execute(
        "UPDATE board_items SET group_id='g-70' WHERE board_id=$1",
        &[&SIBLING],
    )
    .unwrap();
    for (id, group, title) in [(59, "g-20", "Ink"), (57, "g-30", "Stored")] {
        tx.execute(
            "INSERT INTO board_items (id,board_id,group_id,title,position) VALUES ($1,$2,$3,$4,0)",
            &[&Uuid::from_u128(id), &SELECTED, &group, &title],
        )
        .unwrap();
    }
    tx.execute(
        "DELETE FROM board_groups WHERE board_id IN ($1,$2) AND id='main'",
        &[&SELECTED, &SIBLING],
    )
    .unwrap();
    tx.commit().unwrap();
}

// Independent oracle includes inactive rows, which no product read returns.
#[cfg(test)]
pub fn check_groups(f: &Fixture, saved: bool) {
    check_group_items(f, saved, None);
}

#[cfg(test)]
pub fn check_group_items(f: &Fixture, saved: bool, created: Option<(Uuid, &str, i64, &str, Uuid)>) {
    let mut db = f.connect();
    let groups = db.query("SELECT board_id,id,name,position,active FROM board_groups WHERE board_id IN ($1,$2) ORDER BY board_id,id", &[&SELECTED,&SIBLING]).unwrap();
    let expected = [
        (SELECTED, "g-10", "Reserve", 2, true),
        (SELECTED, "g-20", "Work", 2, true),
        (SELECTED, "g-30", "Retired", 0, false),
        (SELECTED, "g-90", "Work", 1, true),
        (SIBLING, "g-70", "Work", 1, true),
    ];
    assert_eq!(groups.len(), expected.len());
    for (r, e) in groups.iter().zip(expected) {
        assert_eq!(
            (
                r.get::<_, Uuid>(0),
                r.get::<_, &str>(1),
                r.get::<_, &str>(2),
                r.get::<_, i64>(3),
                r.get::<_, bool>(4)
            ),
            e
        );
    }
    let items = db.query("SELECT board_id,group_id,id,position,title,revision FROM board_items WHERE board_id IN ($1,$2,$3) ORDER BY id", &[&SELECTED,&SIBLING,&f.board]).unwrap();
    let mut expected = vec![
        (f.board, "main", 17, 1, "Review", 1),
        (f.board, "main", 42, 0, "Review", 1),
        (SELECTED, "g-30", 57, 0, "Stored", 1),
        (
            SELECTED,
            "g-20",
            59,
            0,
            if saved { "Coral" } else { "Ink" },
            if saved { 2 } else { 1 },
        ),
        (SELECTED, "g-90", 61, 2, "Ink", 1),
        (SIBLING, "g-70", 62, 0, "Outline", 1),
        (SELECTED, "g-90", 73, 0, "Sketch", 1),
    ];
    if let Some((id, group, position, title, _)) = created {
        expected.push((SELECTED, group, id.as_u128(), position, title, 1));
        expected.sort_by_key(|e| e.2);
    }
    let provenance = db.query("SELECT id,created_by,creation_title FROM board_items WHERE created_by IS NOT NULL OR creation_title IS NOT NULL", &[]).unwrap();
    assert_eq!(provenance.len(), usize::from(created.is_some()));
    if let Some((id, _, _, title, creator)) = created {
        assert_eq!(provenance[0].get::<_, Uuid>(0), id);
        assert!(provenance[0].get::<_, Uuid>(1) == creator);
        assert_eq!(provenance[0].get::<_, &str>(2), title);
    }
    assert_eq!(items.len(), expected.len());
    for (r, e) in items.iter().zip(expected) {
        assert_eq!(
            (
                r.get::<_, Uuid>(0),
                r.get::<_, &str>(1),
                r.get::<_, Uuid>(2).as_u128(),
                r.get::<_, i64>(3),
                r.get::<_, &str>(4),
                r.get::<_, i64>(5)
            ),
            e
        );
    }
}

pub fn selected_rows() -> Vec<SavedItem> {
    rows(&[(73, "Sketch"), (61, "Ink")])
}
fn rows(items: &[(u128, &str)]) -> Vec<SavedItem> {
    items
        .iter()
        .map(|&(n, title)| SavedItem {
            id: Uuid::from_u128(n),
            title: title.into(),
            revision: 1,
        })
        .collect()
}

#[cfg(test)]
pub struct Selection {
    pub host: Host,
    pub token: Token,
    pub session: StoredSession,
    catalog: Catalog,
}

#[cfg(test)]
impl Selection {
    pub fn new(f: &Fixture) -> Self {
        let password = Uuid::new_v4().simple().to_string();
        let member = f.seed_account("reader", &password, Role::Member);
        let catalog = Catalog::new(f, member);
        let host =
            Host::start_with_navigation(&f.schema.config, BoardId::new(f.board).unwrap()).unwrap();
        let token = boardcraft_account::sign_in(&f.schema.config, "reader", &password).unwrap();
        let session = StoredSession::only(f, member);
        Self {
            host,
            token,
            session,
            catalog,
        }
    }

    pub fn membership(&self, f: &Fixture, joined: bool) {
        self.catalog.membership(f, joined);
    }
    pub fn check(&self, f: &Fixture, joined: bool) {
        assert!(StoredSession::only(f, self.catalog.member) == self.session);
        self.check_rows(f, joined);
    }
    pub fn check_rows(&self, f: &Fixture, joined: bool) {
        self.catalog.check(f, joined);
    }
}

pub struct Catalog {
    member: Uuid,
    workspace: Uuid,
}

impl Catalog {
    pub fn new(f: &Fixture, member: Uuid) -> Self {
        let creator = f.seed_account("creator", &Uuid::new_v4().to_string(), Role::Member);
        let workspace = Uuid::new_v4();
        let mut db = f.connect();
        db.execute("INSERT INTO workspaces(id,creator_id,name,position,active,is_open) VALUES ($1,$2,'Grove',0,true,false)", &[&workspace,&creator]).unwrap();
        f.set_creator(member); // Direct legacy grant: valid target, never a catalog destination.
        for (id, name, pos, items) in [
            (SELECTED, "Review", 2, selected_rows()),
            (HIDDEN, "Hidden", 0, vec![]),
            (SIBLING, "Review", 1, rows(&[(62, "Outline")])),
            (QUARTZ, "Quartz", 0, vec![]),
        ] {
            let mut board = Board::new(BoardId::new(id).unwrap());
            for row in items {
                board
                    .add_item(ItemId::new(row.id).unwrap(), &row.title)
                    .unwrap();
            }
            store::create_board(&mut db, &board).unwrap();
            db.execute("INSERT INTO board_policies(board_id,workspace_id,name,position,kind,creator_id,active) VALUES ($1,$2,$3,$4,'private',$5,true)", &[&id,&workspace,&name,&pos,&creator]).unwrap();
            if id != HIDDEN {
                db.execute(
                    "INSERT INTO board_subscribers VALUES ($1,$2)",
                    &[&id, &member],
                )
                .unwrap();
            }
        }
        let s = Self { member, workspace };
        s.membership(f, true);
        s
    }

    pub fn membership(&self, f: &Fixture, joined: bool) {
        let sql = if joined {
            "INSERT INTO workspace_members VALUES ($1,$2)"
        } else {
            "DELETE FROM workspace_members WHERE workspace_id=$1 AND account_id=$2"
        };
        assert_eq!(
            f.connect()
                .execute(sql, &[&self.workspace, &self.member])
                .unwrap(),
            1
        );
    }

    pub fn check(&self, f: &Fixture, joined: bool) {
        self.check_with_selected(f, joined, &selected_rows());
    }

    pub fn check_with_selected(&self, f: &Fixture, joined: bool, selected: &[SavedItem]) {
        self.check_grants(f, joined);
        f.check_board(SELECTED, selected);
        f.check_board(SIBLING, &rows(&[(62, "Outline")]));
    }

    pub fn check_grants(&self, f: &Fixture, joined: bool) {
        let mut db = f.connect();
        assert_eq!(db.query_one("SELECT EXISTS(SELECT FROM workspace_members WHERE workspace_id=$1 AND account_id=$2)", &[&self.workspace,&self.member]).unwrap().get::<_,bool>(0),joined);
        let subscriptions: Vec<Uuid> = db
            .query(
                "SELECT board_id FROM board_subscribers WHERE account_id=$1 ORDER BY board_id",
                &[&self.member],
            )
            .unwrap()
            .into_iter()
            .map(|r| r.get(0))
            .collect();
        assert_eq!(subscriptions, [SELECTED, SIBLING, QUARTZ]);
    }
}
