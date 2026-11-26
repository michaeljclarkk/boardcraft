use crate::{
    create::{Case, login, rejected},
    group_edit::{self, FAULT, STORED},
    reorder_state::Snapshot,
    selection::{HIDDEN, SELECTED, SIBLING},
};
use boardcraft_board::{BoardId, store};
use boardcraft_mobile_edit_host::Host;
use serde_json::{Value, json};

fn owner() -> Case {
    let c = Case::new();
    group_edit::seed(&c.f);
    c.f.connect()
        .execute("UPDATE accounts SET role='owner' WHERE id=$1", &[&c.member])
        .unwrap();
    c
}
fn path(id: &str) -> String {
    format!("/preview/api/navigation/boards/{SELECTED}/groups/{id}")
}
fn draft(board: i64, group: i64, name: &str, colour: &str) -> Value {
    json!({"board_revision":board,"group_revision":group,"name":name,"colour":colour})
}
fn check(c: &Case, s: &Snapshot, joined: bool, live: bool) {
    assert_eq!(&Snapshot::read(&c.f), s);
    c.catalog.check_grants(&c.f, joined);
    assert_eq!(c.session.present(&c.f), live);
}
pub fn access() {
    let c = owner();
    let mut db = c.f.connect();
    let p = path("g-z");
    let d = draft(7, 1, "Updated", STORED);
    db.execute("INSERT INTO board_groups(board_id,id,name,position,active,deleted) VALUES($1,'deleted','Work',9,false,true)",&[&SELECTED]).unwrap();
    let s = Snapshot::read(&c.f);
    let put = || c.request(Some(&c.pair), "PUT", &p, &d);
    rejected(c.request(None, "PUT", &p, &d), 401);
    for (method, path, status) in [
        ("POST", p.clone(), 404),
        ("PUT", p.replace("/navigation/boards", "/boards"), 404),
        ("PUT", p.replace("/preview/api", ""), 403),
        (
            "PUT",
            p.replace(&SELECTED.to_string(), &HIDDEN.to_string()),
            403,
        ),
        ("PUT", path("g-peer"), 404),
        ("PUT", path("absent"), 404),
        ("PUT", path("g-hidden"), 404),
        ("PUT", path("deleted"), 404),
    ] {
        rejected(c.request(Some(&c.pair), method, &path, &d), status);
    }
    db.execute(
        "DELETE FROM board_subscribers WHERE board_id=$1 AND account_id=$2",
        &[&SELECTED, &c.member],
    )
    .unwrap();
    rejected(put(), 403);
    db.execute(
        "INSERT INTO board_subscribers VALUES($1,$2)",
        &[&SELECTED, &c.member],
    )
    .unwrap();
    c.catalog.membership(&c.f, false);
    rejected(put(), 403);
    check(&c, &s, false, true);
    c.catalog.membership(&c.f, true);
    for sql in [
        "UPDATE board_policies SET active=$2 WHERE board_id=$1",
        "UPDATE workspaces SET active=$2 WHERE id=(SELECT workspace_id FROM board_policies WHERE board_id=$1)",
    ] {
        db.execute(sql, &[&SELECTED, &false]).unwrap();
        rejected(put(), 403);
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
        rejected(put(), 403);
        check(&c, &s, true, true);
    }
    c.session.revoke(&c.f);
    rejected(put(), 401);
    check(&c, &s, true, false);
}

pub fn edits() {
    let c = owner();
    let mut db = c.f.connect();
    let p = path("g-z");
    let put = |d: &Value| c.request(Some(&c.pair), "PUT", &p, d);
    db.execute(
        "INSERT INTO board_groups(board_id,id,name,position,active) VALUES($1,'g-z','Work',3,true)",
        &[&SIBLING],
    )
    .unwrap();
    let old = Snapshot::read(&c.f);
    let d = draft(7, 1, "Work", STORED);
    let name = "😀".repeat(80);
    for (key, value, code) in [
        ("board_revision", json!(0), 422),
        ("group_revision", json!(0), 422),
        ("board_revision", json!(6), 409),
        ("group_revision", json!(2), 409),
        ("name", json!("  "), 422),
        ("name", json!(format!("{name}a")), 422),
        ("colour", json!("#12345z"), 422),
        ("colour", json!("#12345"), 422),
        ("colour", json!("123456"), 422),
        ("position", json!(3), 400),
    ] {
        let mut bad = d.clone();
        bad[key] = value;
        rejected(put(&bad), code);
        check(&c, &old, true, true);
    }
    assert_eq!(put(&d).status, 200);
    let s = old.edited_group(&c.f, "g-z", "Work", STORED);
    rejected(put(&d), 409);
    let a = draft(8, 2, &format!(" {name} "), "#AbcDEF");
    let b = draft(8, 2, "Peer", "#00c875");
    db.batch_execute(FAULT).unwrap();
    rejected(put(&a), 500);
    check(&c, &s, true, true);
    db.batch_execute("DROP TRIGGER refuse_edit ON board_groups")
        .unwrap();
    let rival = Host::start_preview(&c.f.schema.config, BoardId::new(c.f.board).unwrap()).unwrap();
    let pair = login(rival.port, "reader", &c.password);
    let barrier = std::sync::Barrier::new(2);
    let codes = std::thread::scope(|scope| {
        let t = scope.spawn(|| {
            barrier.wait();
            Case::request_at(rival.port, Some(&pair), "PUT", &p, &a).status
        });
        barrier.wait();
        [put(&b).status, t.join().unwrap()]
    });
    let winner = match codes {
        [200, 409] => &b,
        [409, 200] => &a,
        _ => panic!("unexpected edit statuses {codes:?}"),
    };
    let s = s.edited_group(
        &c.f,
        "g-z",
        winner["name"].as_str().unwrap().trim(),
        winner["colour"].as_str().unwrap(),
    );
    rejected(put(winner), 409);
    check(&c, &s, true, true);
    let max = draft(9, 3, &format!(" {name} "), "#AbcDEF");
    assert_eq!(put(&max).status, 200);
    s.edited_group(&c.f, "g-z", &name, "#AbcDEF");
    let board = BoardId::new(SELECTED).unwrap();
    let (rev, groups) = store::load_group_snapshot(&mut c.f.connect(), board)
        .ok()
        .unwrap();
    assert_eq!(rev, 10);
    assert_eq!(
        groups
            .iter()
            .map(|g| (g.id.as_str(), g.revision))
            .collect::<Vec<_>>(),
        [("g-old", 1), ("g-z", 4), ("g-end", 1)]
    );
    assert_eq!(groups[1].name, name);
    assert_eq!(groups[1].colour, "#AbcDEF");
    let ids = |rows: Vec<store::StoredItem>| {
        rows.into_iter()
            .map(|r| r.id.as_uuid().as_u128())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        groups.into_iter().map(|g| ids(g.items)).collect::<Vec<_>>(),
        [vec![59], vec![61, 63], vec![]]
    );
    assert_eq!(
        ids(store::load_items(&mut c.f.connect(), board)
            .unwrap()
            .unwrap()),
        [59, 61, 63]
    );
    db.execute("INSERT INTO board_groups(board_id,id,name,position,active) VALUES($1,'main','Items',0,true)",&[&SELECTED]).unwrap();
    let s = Snapshot::read(&c.f);
    assert_eq!(
        c.request(
            Some(&c.pair),
            "PUT",
            &path("main"),
            &draft(10, 1, "Main", STORED)
        )
        .status,
        200
    );
    s.edited_group(&c.f, "main", "Main", STORED);
}
