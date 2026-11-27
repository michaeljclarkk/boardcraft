use crate::{
    create::{Case, rejected},
    group_edit_checks::owner,
    reorder_state::Snapshot,
    selection::{SELECTED, SIBLING},
};
use boardcraft_board::{BoardId, store};
use boardcraft_mobile_edit_client::EditClient;
use serde_json::{Value, json};
use uuid::Uuid;

const A: &str = "g-old";
const B: &str = "g-z";
const C: &str = "g-end";
fn path() -> String {
    format!("/web/api/navigation/boards/{SELECTED}")
}
fn draft(rev: i64, ids: &[&str], collapsed: bool) -> Value {
    json!({"view_revision":rev,"group_ids":ids,"collapsed":collapsed})
}
fn post(c: &Case, d: Value) -> crate::browser_transport::Reply {
    c.request(Some(&c.pair), "POST", &(path() + "/main-table-fold"), &d)
}

fn reads(c: &Case, client: &EditClient, order: &[&str], folded: &[&str]) {
    for (board, ids) in [(SELECTED, order), (SIBLING, &["g-peer"][..])] {
        let reply = c.request(
            Some(&c.pair),
            "GET",
            &format!("/web/api/navigation/boards/{board}"),
            &Value::Null,
        );
        assert_eq!(reply.status, 200);
        let groups: Vec<Value> = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(groups.len(), ids.len());
        let mut roots = Vec::new();
        for (g, &id) in groups.iter().zip(ids) {
            assert_eq!(g["id"], id);
            assert_eq!(g["collapsed"], folded.contains(&id));
            let children: &[u128] = match id {
                A => &[59],
                B => &[61, 63],
                C => &[],
                "g-peer" => &[62],
                _ => unreachable!(),
            };
            let rows: Vec<_> = children
                .iter()
                .map(|n| json!({"id":Uuid::from_u128(*n),"title":"Ink","revision":1}))
                .collect();
            assert_eq!(g["items"], json!(rows));
            roots.extend(rows);
        }
        assert_eq!(
            serde_json::to_value(client.read_selected_board(board).unwrap()).unwrap(),
            json!(roots)
        );
    }
}

pub fn scope() {
    let c = owner();
    let mut db = c.f.connect();
    let mut s = Snapshot::read(&c.f);
    db.batch_execute(
        "DROP TABLE main_table_folds,main_tables; DELETE FROM rustday_migrations WHERE version=11",
    )
    .unwrap();
    store::migrate(&mut db).unwrap();
    assert_eq!(s, Snapshot::read(&c.f));
    let token = boardcraft_account::sign_in(&c.f.schema.config, "reader", &c.password).unwrap();
    let client = EditClient::with_bearer(c.host.port, token.encoded()).unwrap();
    reads(&c, &client, &[A, B, C], &[]);
    for (d, status) in [
        (json!({"collapsed":true}), 400),
        (draft(0, &[B], true), 422),
        (draft(1, &[], true), 422),
        (draft(1, &[B, B], false), 422),
        (draft(1, &[""], true), 422),
        (draft(1, &vec![A; 501], true), 413),
        (draft(1, &[A, "g-peer"], true), 404),
        (draft(1, &["g-hidden"], true), 404),
        (draft(1, &["Work"], true), 404),
    ] {
        rejected(post(&c, d), status);
    }
    assert_eq!(s, Snapshot::read(&c.f));
    assert_eq!(post(&c, draft(1, &[B], true)).status, 200);
    s = s.folded(&c.f, 2, &[B]);
    let bac = [B, A, C];
    assert_eq!(
        c.request(
            Some(&c.pair),
            "POST",
            &(path() + "/group-order"),
            &json!({"board_revision":8,"group_ids":bac})
        )
        .status,
        200
    );
    s = s.reordered_groups(&c.f, &bac);
    reads(&c, &client, &bac, &[B]);
    rejected(post(&c, draft(1, &bac, true)), 409);
    assert_eq!(s, Snapshot::read(&c.f));
    // An archived section can already have a saved fold. Neither menu verb
    // is a replacement of the entire presentation: it targets displayed IDs.
    db.execute(
        "INSERT INTO main_table_folds VALUES ($1,'g-hidden')",
        &[&SELECTED],
    )
    .unwrap();
    s = Snapshot::read(&c.f);
    assert_eq!(post(&c, draft(2, &bac, true)).status, 200);
    s = s.folded(&c.f, 3, &[B, A, C, "g-hidden"]);
    reads(&c, &client, &bac, &bac);
    assert_eq!(post(&c, draft(3, &bac, false)).status, 200);
    s.folded(&c.f, 4, &["g-hidden"]);
    reads(&c, &client, &bac, &[]);
    let board = BoardId::new(Uuid::new_v4()).unwrap();
    store::create_board(&mut db, &boardcraft_board::Board::new(board)).unwrap();
    let (b, v, g) = store::load_main_table(&mut db, board).ok().unwrap();
    assert_eq!((b, v), (1, 1));
    assert_eq!(g.len(), 1);
    assert!(!g[0].collapsed);
}

pub fn access() {
    let c = owner();
    let mut db = c.f.connect();
    let s = Snapshot::read(&c.f);
    let save = || post(&c, draft(1, &[B], true));
    db.execute(
        "UPDATE accounts SET role='member' WHERE id=$1",
        &[&c.member],
    )
    .unwrap();
    db.batch_execute("CREATE FUNCTION refuse_fold() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER refuse_fold AFTER INSERT ON main_tables DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION refuse_fold()").unwrap();
    rejected(save(), 500);
    assert_eq!(s, Snapshot::read(&c.f));
    db.batch_execute("DROP TRIGGER refuse_fold ON main_tables")
        .unwrap();
    c.catalog.membership(&c.f, false);
    rejected(save(), 403);
    rejected(c.request(Some(&c.pair), "GET", &path(), &Value::Null), 403);
    c.catalog.check_grants(&c.f, false);
    c.catalog.membership(&c.f, true);
    db.execute(
        "DELETE FROM board_subscribers WHERE board_id=$1 AND account_id=$2",
        &[&SELECTED, &c.member],
    )
    .unwrap();
    rejected(save(), 403);
    db.execute(
        "INSERT INTO board_subscribers VALUES ($1,$2)",
        &[&SELECTED, &c.member],
    )
    .unwrap();
    assert_eq!(s, Snapshot::read(&c.f));
    assert_eq!(save().status, 200);
    let s = s.folded(&c.f, 2, &[B]);
    db.execute(
        "UPDATE accounts SET role='viewer' WHERE id=$1",
        &[&c.member],
    )
    .unwrap();
    rejected(post(&c, draft(2, &[B], false)), 403);
    c.catalog.check_grants(&c.f, true);
    c.session.revoke(&c.f);
    rejected(save(), 401);
    assert_eq!(s, Snapshot::read(&c.f));
}
