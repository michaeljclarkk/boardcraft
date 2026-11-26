use crate::{
    create::{Case, login, rejected},
    group_create::{self, BLUE, FAULT, GREEN},
    reorder_state::Snapshot,
    selection::{HIDDEN, SELECTED, SIBLING},
};
use boardcraft_board::{BoardId, store};
use boardcraft_mobile_edit_host::Host;
use serde_json::{Value, json};
use uuid::Uuid;

fn owner() -> Case {
    let c = Case::new();
    group_create::seed(&c.f);
    c.f.connect()
        .execute("UPDATE accounts SET role='owner' WHERE id=$1", &[&c.member])
        .unwrap();
    c
}
fn path(board: Uuid) -> String {
    format!("/preview/api/navigation/boards/{board}/groups")
}
fn draft() -> Value {
    json!({"id":format!("g-{}",Uuid::new_v4()),"name":"New group","colour":BLUE,"board_revision":1})
}
fn post(c: &Case, d: &Value) -> crate::browser_transport::Reply {
    c.request(Some(&c.pair), "POST", &path(SELECTED), d)
}
fn check(c: &Case, s: &Snapshot, joined: bool, live: bool) {
    assert_eq!(&Snapshot::read(&c.f), s);
    c.catalog.check_grants(&c.f, joined);
    assert_eq!(c.session.present(&c.f), live);
}

pub fn access() {
    let c = owner();
    let d = draft();
    let p = path(SELECTED);
    let mut db = c.f.connect();
    let s = Snapshot::read(&c.f);
    rejected(c.request(None, "POST", &p, &d), 401);
    rejected(c.request(Some(&c.pair), "POST", &path(HIDDEN), &d), 403);
    rejected(
        c.request(
            Some(&c.pair),
            "POST",
            &p.replace("/navigation/boards", "/boards"),
            &d,
        ),
        404,
    );
    rejected(
        c.request(Some(&c.pair), "POST", &p.replace("/preview/api", ""), &d),
        403,
    );
    rejected(c.request(Some(&c.pair), "PUT", &p, &d), 404);
    rejected(post(&c, &json!({"id":"bad"})), 400);
    check(&c, &s, true, true);
    db.execute(
        "DELETE FROM board_subscribers WHERE board_id=$1 AND account_id=$2",
        &[&SELECTED, &c.member],
    )
    .unwrap();
    rejected(post(&c, &d), 403);
    assert_eq!(Snapshot::read(&c.f), s);
    db.execute(
        "INSERT INTO board_subscribers VALUES($1,$2)",
        &[&SELECTED, &c.member],
    )
    .unwrap();
    assert_eq!(post(&c, &d).status, 200);
    let s = s.created_group(&c.f, d["id"].as_str().unwrap(), "New group", BLUE, 3);
    c.catalog.membership(&c.f, false);
    rejected(post(&c, &d), 403);
    check(&c, &s, false, true);
    c.catalog.membership(&c.f, true);
    for sql in [
        "UPDATE board_policies SET active=$2 WHERE board_id=$1",
        "UPDATE workspaces SET active=$2 WHERE id=(SELECT workspace_id FROM board_policies WHERE board_id=$1)",
    ] {
        db.execute(sql, &[&SELECTED, &false]).unwrap();
        rejected(post(&c, &d), 403);
        check(&c, &s, true, true);
        db.execute(sql, &[&SELECTED, &true]).unwrap();
    }
    db.execute(
        "UPDATE board_policies SET creator_id=$2 WHERE board_id=$1",
        &[&SELECTED, &c.member],
    )
    .unwrap();
    for role in ["member", "viewer"] {
        db.execute(
            "UPDATE accounts SET role=$2 WHERE id=$1",
            &[&c.member, &role],
        )
        .unwrap();
        rejected(post(&c, &d), 403);
        rejected(post(&c, &draft()), 403);
        check(&c, &s, true, true);
    }
    c.session.revoke(&c.f);
    rejected(post(&c, &d), 401);
    check(&c, &s, true, false);
}

pub fn round_trip() {
    let c = owner();
    let mut db = c.f.connect();
    let old = Snapshot::read(&c.f);
    db.batch_execute("ALTER TABLE board_groups DROP COLUMN colour; DELETE FROM rustday_migrations WHERE version=10").unwrap();
    for _ in 0..2 {
        store::migrate(&mut c.f.connect()).unwrap();
        check(&c, &old, true, true);
    }
    let mut a = draft();
    let mut b = draft();
    let name = "😀".repeat(80);
    a["name"] = json!(format!(" {name} "));
    b["name"] = a["name"].clone();
    for (key, bad) in [
        ("name", json!("   ")),
        ("name", json!(format!("{name}a"))),
        ("colour", json!("#12345z")),
        ("colour", json!("#12345")),
        ("colour", json!("123456")),
        ("id", json!("not-a-group")),
    ] {
        let mut bad_draft = a.clone();
        bad_draft[key] = bad;
        rejected(post(&c, &bad_draft), 422);
        check(&c, &old, true, true);
    }
    // A matching key on the sibling cannot acknowledge a stale request here.
    db.execute(
        "INSERT INTO board_groups(board_id,id,name,position,active) VALUES($1,$2,'Foreign',2,true)",
        &[&SIBLING, &a["id"].as_str().unwrap()],
    )
    .unwrap();
    let s = Snapshot::read(&c.f);
    let mut stale = a.clone();
    stale["board_revision"] = json!(2);
    rejected(post(&c, &stale), 409);
    check(&c, &s, true, true);
    let rival = Host::start_preview(&c.f.schema.config, BoardId::new(c.f.board).unwrap()).unwrap();
    let pair = login(rival.port, "reader", &c.password);
    let barrier = std::sync::Barrier::new(2);
    let codes = std::thread::scope(|scope| {
        let t = scope.spawn(|| {
            barrier.wait();
            Case::request_at(rival.port, Some(&pair), "POST", &path(SELECTED), &b).status
        });
        barrier.wait();
        [post(&c, &a).status, t.join().unwrap()]
    });
    let mut winner = match codes {
        [200, 409] => a,
        [409, 200] => b,
        _ => panic!("unexpected race statuses {codes:?}"),
    };
    let id = winner["id"].as_str().unwrap().to_owned();
    let s = s.created_group(&c.f, &id, &name, BLUE, 3);
    assert_eq!(post(&c, &winner).status, 200);
    winner["name"] = json!("Different");
    winner["colour"] = json!(GREEN);
    assert_eq!(post(&c, &winner).status, 200);
    check(&c, &s, true, true);
    let board = BoardId::new(SELECTED).unwrap();
    let (rev, groups) = store::load_group_snapshot(&mut c.f.connect(), board)
        .ok()
        .unwrap();
    assert_eq!(rev, 2);
    assert_eq!(
        groups.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(),
        ["g-old", &id, "g-z", "g-end"]
    );
    assert!(groups[1].items.is_empty());
    assert_eq!(groups[1].name, name);
    assert_eq!(groups[1].colour, BLUE);
    let flat = store::load_items(&mut c.f.connect(), board)
        .unwrap()
        .unwrap();
    assert_eq!(
        flat.iter()
            .map(|r| r.id.as_uuid().as_u128())
            .collect::<Vec<_>>(),
        [59, 61]
    );
    let mut next = draft();
    next["board_revision"] = json!(2);
    db.batch_execute(FAULT).unwrap();
    rejected(post(&c, &next), 500);
    check(&c, &s, true, true);
    db.batch_execute("DROP TRIGGER refuse_group ON board_groups")
        .unwrap();
    db.execute("INSERT INTO board_groups(board_id,id,name,position,active) SELECT $1,'empty-'||n,'Empty',7,true FROM generate_series(1,496)n",&[&SELECTED]).unwrap();
    let full = Snapshot::read(&c.f);
    rejected(post(&c, &next), 413);
    check(&c, &full, true, true);
}
