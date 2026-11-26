use super::{Caller, Fixture, Role, rows};
use boardcraft_board::BoardId;
use boardcraft_mobile_edit_client::{BoardSummary, EditClient};
use boardcraft_mobile_edit_host::Host;
use uuid::Uuid;

pub(super) struct Catalog {
    pub member: Caller,
    pub viewer: Caller,
    pub ash: BoardSummary,
    pub amber: BoardSummary,
    cedar: BoardSummary,
    pub quartz: BoardSummary,
    pub pine: BoardSummary,
}

impl Catalog {
    pub fn host(&self, f: &Fixture) -> Host {
        Host::start_with_navigation(&f.schema.config, BoardId::new(self.ash.id).unwrap()).unwrap()
    }

    pub fn open_for_member(&self) -> Vec<BoardSummary> {
        vec![self.amber.clone(), self.cedar.clone(), self.quartz.clone()]
    }

    pub fn joined_for_member(&self) -> Vec<BoardSummary> {
        vec![
            self.ash.clone(),
            self.amber.clone(),
            self.cedar.clone(),
            self.quartz.clone(),
            self.pine.clone(),
        ]
    }

    pub fn seed(f: &Fixture) -> Self {
        let member = Caller::sign_in(f, Role::Member);
        let viewer = Caller::sign_in(f, Role::Viewer);
        let other = f.seed_account("other", &Uuid::new_v4().to_string(), Role::Member);
        let mut db = f.connect();
        let mut workspace = |name: &str, creator: Uuid, open: bool, active: bool, pos: i32| {
            let id = Uuid::new_v4();
            db.execute(
                "INSERT INTO workspaces(id,creator_id,name,position,active,is_open)
                 VALUES ($1,$2,$3,$4,$5,$6)",
                &[&id, &creator, &name, &pos, &active, &open],
            )
            .unwrap();
            id
        };
        let open = workspace("Orchard", other, true, true, 0);
        let closed = workspace("Grove", viewer.id, false, true, 1);
        let inactive = workspace("Park", other, true, false, 2);
        let mut board = |ws: Uuid,
                         name: &str,
                         pos: i32,
                         kind: &str,
                         creator: Uuid,
                         active: bool,
                         id: Uuid| {
            if id != f.board {
                db.execute("INSERT INTO boards(id) VALUES ($1)", &[&id])
                    .unwrap();
            }
            db.execute(
                "INSERT INTO board_policies(board_id,workspace_id,name,position,kind,creator_id,active)
                 VALUES ($1,$2,$3,$4,$5,$6,$7)",
                &[&id, &ws, &name, &pos, &kind, &creator, &active],
            ).unwrap();
            BoardSummary {
                id,
                workspace_id: ws,
                name: name.into(),
            }
        };
        let quartz = board(open, "Quartz", 2, "main", other, true, Uuid::new_v4());
        let pine = board(closed, "Pine", 3, "main", other, true, Uuid::new_v4());
        let cedar = board(open, "Cedar", 1, "shareable", other, true, Uuid::new_v4());
        let amber = board(open, "Amber", 1, "private", member.id, true, Uuid::new_v4());
        let ash = board(closed, "Ash", 0, "private", other, true, f.board);
        board(closed, "Birch", 1, "private", other, true, Uuid::new_v4());
        board(open, "Dormant", 0, "main", other, false, Uuid::new_v4());
        board(inactive, "Willow", 0, "main", other, true, Uuid::new_v4());
        db.execute("INSERT INTO boards(id) VALUES ($1)", &[&Uuid::new_v4()])
            .unwrap();
        for (id, actor) in [
            (ash.id, member.id),
            (ash.id, viewer.id),
            (cedar.id, member.id),
            (amber.id, viewer.id),
        ] {
            db.execute(
                "INSERT INTO board_subscribers VALUES ($1,$2)",
                &[&id, &actor],
            )
            .unwrap();
        }
        Self {
            member,
            viewer,
            ash,
            amber,
            cedar,
            quartz,
            pine,
        }
    }

    // Independent of HTTP: compare the identities of every membership, board
    // subscription and signed-in account, as well as the target's ordered rows.
    pub fn check_state(&self, f: &Fixture, role: &str, joined: bool, signed_in: bool) {
        let mut db = f.connect();
        let member = self.member.id;
        let viewer = self.viewer.id;
        assert_eq!(
            db.query_one("SELECT role FROM accounts WHERE id=$1", &[&member])
                .unwrap()
                .get::<_, String>(0),
            role
        );
        let sessions: Vec<Uuid> = db
            .query(
                "SELECT account_id FROM account_sessions ORDER BY account_id",
                &[],
            )
            .unwrap()
            .into_iter()
            .map(|r| r.get(0))
            .collect();
        let mut expected = vec![viewer];
        if signed_in {
            expected.push(member);
        }
        expected.sort();
        assert_eq!(sessions, expected);
        let mut pairs = |query| {
            db.query(query, &[])
                .unwrap()
                .into_iter()
                .map(|r| (r.get::<_, Uuid>(0), r.get::<_, Uuid>(1)))
                .collect::<Vec<_>>()
        };
        let memberships = pairs("SELECT workspace_id,account_id FROM workspace_members");
        assert_eq!(
            memberships,
            if joined {
                vec![(self.ash.workspace_id, member)]
            } else {
                vec![]
            }
        );
        let subscriptions =
            pairs("SELECT board_id,account_id FROM board_subscribers ORDER BY board_id,account_id");
        let mut expected = [
            (self.ash.id, member),
            (self.ash.id, viewer),
            (self.cedar.id, member),
            (self.amber.id, viewer),
        ];
        expected.sort();
        assert_eq!(subscriptions, expected);
        f.check_store(&rows("Review", 1));
    }

    pub fn check_limits(&self, f: &Fixture, viewer: &EditClient) {
        // 1,999 earlier visible workspaces leave room for Orchard, not Grove.
        // Neither the inactive workspace nor its high-ranked board can use a slot.
        let mut db = f.connect();
        db.batch_execute("UPDATE workspaces SET position=-2 WHERE NOT active")
            .unwrap();
        let workspaces: Vec<_> = (0..1999).map(|_| Uuid::new_v4()).collect();
        db.execute(
            "INSERT INTO workspaces(id,creator_id,name,position,active,is_open)
            SELECT id,$2,'Early',-1,true,true FROM unnest($1::uuid[]) id",
            &[&workspaces, &self.member.id],
        )
        .unwrap();
        let ids: Vec<_> = (0..2000).map(|_| Uuid::new_v4()).collect();
        db.execute("INSERT INTO boards SELECT unnest($1::uuid[])", &[&ids])
            .unwrap();
        db.execute(
            "INSERT INTO board_policies(board_id,workspace_id,name,position,creator_id,kind)
            SELECT id,$2,'Sample',n+3,$3,'main' FROM unnest($1::uuid[]) WITH ORDINALITY b(id,n)",
            &[&ids, &workspaces[0], &self.member.id],
        )
        .unwrap();
        let mut expected = vec![self.amber.clone(), self.quartz.clone()];
        expected.extend(ids[..1998].iter().map(|&id| BoardSummary {
            id,
            workspace_id: workspaces[0],
            name: "Sample".into(),
        }));
        assert_eq!(viewer.list_boards().unwrap(), expected);
    }
}
