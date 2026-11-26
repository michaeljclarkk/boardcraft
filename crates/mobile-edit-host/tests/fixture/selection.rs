//! Shared database oracle for the HTTP and graphical destination checks.
use crate::{fixture::Fixture, stored_session::StoredSession};
use boardcraft_account::{Role, Token};
use boardcraft_board::{Board, BoardId, ItemId, store};
use boardcraft_mobile_edit_client::SavedItem;
use boardcraft_mobile_edit_host::Host;
use uuid::Uuid;

pub const QUARTZ: Uuid = Uuid::from_u128(203);
pub const SIBLING: Uuid = Uuid::from_u128(202);
pub const SELECTED: Uuid = Uuid::from_u128(201);
pub const HIDDEN: Uuid = Uuid::from_u128(204);

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

pub struct Selection {
    pub host: Host,
    pub token: Token,
    pub session: StoredSession,
    member: Uuid,
    workspace: Uuid,
}

impl Selection {
    pub fn new(f: &Fixture) -> Self {
        let password = Uuid::new_v4().simple().to_string();
        let member = f.seed_account("reader", &password, Role::Member);
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
        let host =
            Host::start_with_navigation(&f.schema.config, BoardId::new(f.board).unwrap()).unwrap();
        let token = boardcraft_account::sign_in(&f.schema.config, "reader", &password).unwrap();
        let session = StoredSession::only(f, member);
        let s = Self {
            host,
            token,
            session,
            member,
            workspace,
        };
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
        assert!(StoredSession::only(f, self.member) == self.session);
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
        f.check_board(SELECTED, &selected_rows());
        f.check_board(SIBLING, &rows(&[(62, "Outline")]));
    }
}
