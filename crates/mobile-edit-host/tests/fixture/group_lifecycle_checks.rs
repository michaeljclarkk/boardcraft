use crate::{
    create::{Case, rejected},
    lifecycle,
    reorder_state::Snapshot,
    selection::{HIDDEN, SELECTED, SIBLING},
};
use boardcraft_board::{BoardId, store};
use serde_json::{Value, json};
use uuid::Uuid;

fn owner() -> Case {
    let c = Case::new();
    c.f.connect()
        .execute("UPDATE accounts SET role='owner' WHERE id=$1", &[&c.member])
        .unwrap();
    lifecycle::seed_group_recovery(&c.f, c.member);
    c
}
fn board() -> String {
    format!("/preview/api/navigation/boards/{SELECTED}")
}
fn path(id: &str, action: &str) -> String {
    format!("{}/groups/{id}/{action}", board())
}
fn get(c: &Case, suffix: &str) -> Value {
    let r = c.request(
        Some(&c.pair),
        "GET",
        &format!("{}{suffix}", board()),
        &Value::Null,
    );
    assert_eq!(r.status, 200);
    serde_json::from_str(&r.body).unwrap()
}
fn check(c: &Case, s: &Snapshot, joined: bool, live: bool) {
    assert_eq!(&Snapshot::read(&c.f), s);
    c.catalog.check_grants(&c.f, joined);
    assert_eq!(c.session.present(&c.f), live);
}

pub fn access() {
    let c = owner();
    let mut db = c.f.connect();
    db.execute("INSERT INTO board_groups(board_id,id,name,position,active) VALUES($1,'g-90','Other',0,false)",&[&SIBLING]).unwrap();
    db.execute("INSERT INTO board_groups(board_id,id,name,position,active,deleted) VALUES($1,'gone','Old',3,false,true)",&[&SELECTED]).unwrap();
    let s = Snapshot::read(&c.f);
    let draft = json!({"board_revision":2});
    let endpoints = [
        ("GET", format!("{}/archive", board())),
        ("POST", path("g-90", "archive")),
        ("POST", path("g-30", "restore")),
    ];
    let denied = |code, joined, live| {
        for (m, p) in &endpoints {
            rejected(c.request(Some(&c.pair), m, p, &draft), code);
            check(&c, &s, joined, live);
        }
    };
    for (m, p) in &endpoints {
        rejected(c.request(None, m, p, &draft), 401);
        check(&c, &s, true, true);
        rejected(
            c.request(
                Some(&c.pair),
                m,
                &p.replace(&SELECTED.to_string(), &HIDDEN.to_string()),
                &draft,
            ),
            403,
        );
        check(&c, &s, true, true);
        rejected(
            c.request(
                Some(&c.pair),
                m,
                &p.replace("/navigation/boards", "/boards"),
                &draft,
            ),
            404,
        );
        check(&c, &s, true, true);
        rejected(
            c.request(Some(&c.pair), m, &p.replace("/preview/api", ""), &draft),
            403,
        );
        check(&c, &s, true, true);
    }
    c.catalog.membership(&c.f, false);
    denied(403, false, true);
    c.catalog.membership(&c.f, true);
    for sql in [
        "UPDATE board_policies SET active=$2 WHERE board_id=$1",
        "UPDATE workspaces SET active=$2 WHERE id=(SELECT workspace_id FROM board_policies WHERE board_id=$1)",
    ] {
        db.execute(sql, &[&SELECTED, &false]).unwrap();
        denied(403, true, true);
        db.execute(sql, &[&SELECTED, &true]).unwrap();
    }
    for (id, action, code) in [
        ("g-70", "archive", 404),
        ("absent", "restore", 404),
        ("g-90", "restore", 422),
        ("g-30", "archive", 422),
        ("gone", "restore", 422),
        ("gone", "archive", 422),
    ] {
        rejected(
            c.request(Some(&c.pair), "POST", &path(id, action), &draft),
            code,
        );
        check(&c, &s, true, true);
    }
    db.execute(
        "UPDATE board_policies SET creator_id=$1 WHERE board_id=$2",
        &[&c.member, &SELECTED],
    )
    .unwrap();
    for role in ["member", "viewer"] {
        db.execute(
            "UPDATE accounts SET role=$2 WHERE id=$1",
            &[&c.member, &role],
        )
        .unwrap();
        let p = get(&c, "/archive");
        assert_eq!(p["can_manage"], false);
        assert_eq!(p["groups"][0]["id"], "g-30");
        check(&c, &s, true, true);
        for (m, path) in &endpoints[1..] {
            rejected(c.request(Some(&c.pair), m, path, &draft), 403);
            check(&c, &s, true, true);
        }
    }
    c.session.revoke(&c.f);
    denied(401, true, false);
}

pub fn round_trip() {
    let c = owner();
    let mut db = c.f.connect();
    db.execute("INSERT INTO board_groups(board_id,id,name,position,active,deleted) VALUES($1,'gone','Old',3,false,true),($2,'g-90','Other',0,true,false)",&[&SELECTED,&SIBLING]).unwrap();
    let mut s = Snapshot::read(&c.f);
    // Remove precisely the v9 additions, leaving populated v8 tables and the live session.
    db.batch_execute("DROP INDEX archived_groups_by_position; ALTER TABLE board_groups DROP COLUMN revision; DELETE FROM rustday_migrations WHERE version=9").unwrap();
    for _ in 0..2 {
        store::migrate(&mut c.f.connect()).unwrap();
        check(&c, &s, true, true);
    }
    let post = |id, action, b| {
        c.request(
            Some(&c.pair),
            "POST",
            &path(id, action),
            &json!({"board_revision":b}),
        )
    };
    let rival = boardcraft_mobile_edit_host::Host::start_preview(
        &c.f.schema.config,
        BoardId::new(c.f.board).unwrap(),
    )
    .unwrap();
    let pair = crate::create::login(rival.port, "reader", &c.password);
    let barrier = std::sync::Barrier::new(2);
    let mut codes = std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            barrier.wait();
            Case::request_at(
                rival.port,
                Some(&pair),
                "POST",
                &path("g-90", "archive"),
                &json!({"board_revision":2}),
            )
            .status
        });
        barrier.wait();
        [post("g-90", "archive", 2).status, worker.join().unwrap()]
    });
    codes.sort();
    assert_eq!(codes, [200, 409]);
    s = s.group_transition(&c.f, "g-90", false);
    check(&c, &s, true, true);
    rejected(post("g-90", "archive", 2), 409);
    check(&c, &s, true, true);
    let grid = |expected: &[(&str, &[u128])]| {
        let b = BoardId::new(SELECTED).unwrap();
        let groups = store::load_groups(&mut c.f.connect(), b).ok().unwrap();
        assert_eq!(groups.len(), expected.len());
        for (g, (id, ids)) in groups.iter().zip(expected) {
            assert_eq!(g.id, *id);
            assert_eq!(
                g.items
                    .iter()
                    .map(|i| i.id.as_uuid().as_u128())
                    .collect::<Vec<_>>(),
                *ids
            );
        }
        assert_eq!(
            store::load_items(&mut c.f.connect(), b)
                .unwrap()
                .unwrap()
                .iter()
                .map(|i| i.id.as_uuid().as_u128())
                .collect::<Vec<_>>(),
            expected
                .iter()
                .flat_map(|(_, ids)| ids.iter().copied())
                .collect::<Vec<_>>()
        );
    };
    grid(&[("g-20", &[59]), ("g-10", &[])]);
    let p = get(&c, "/archive");
    assert_eq!(p["total"], 6);
    assert!(
        p["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["restorable"] == false)
    );
    for (suffix, draft, code) in [
        (
            "/items/move-selection".into(),
            json!({"operation_id":Uuid::new_v4(),"board_revision":3,"group_id":"g-90","items":[{"id":Uuid::from_u128(59),"revision":1}]}),
            422,
        ),
        (
            format!("/items/{}/restore", Uuid::from_u128(67)),
            json!({"operation_id":Uuid::new_v4(),"board_revision":3,"item_revision":2}),
            404,
        ),
    ] {
        rejected(
            c.request(
                Some(&c.pair),
                "POST",
                &format!("{}{suffix}", board()),
                &draft,
            ),
            code,
        );
        check(&c, &s, true, true);
    }
    db.execute("INSERT INTO board_groups(board_id,id,name,position,active) SELECT $1,'extra-'||n,'Empty',4,true FROM generate_series(1,498)n", &[&SELECTED]).unwrap();
    let full = Snapshot::read(&c.f);
    rejected(post("g-90", "restore", 3), 413);
    check(&c, &full, true, true);
    db.execute(
        "DELETE FROM board_groups WHERE board_id=$1 AND id LIKE 'extra-%'",
        &[&SELECTED],
    )
    .unwrap();
    check(&c, &s, true, true);
    assert_eq!(post("g-90", "restore", 3).status, 200);
    s = s.group_transition(&c.f, "g-90", true);
    check(&c, &s, true, true);
    grid(&[("g-20", &[59]), ("g-90", &[73, 61, 77]), ("g-10", &[])]);
    assert_eq!(post("g-10", "archive", 4).status, 200);
    s = s.group_transition(&c.f, "g-10", false);
    check(&c, &s, true, true);
    db.execute("INSERT INTO board_groups(board_id,id,name,position,active) SELECT $1,'extra-'||lpad(n::text,3,'0'),'Empty',1,false FROM generate_series(1,100)n", &[&SELECTED]).unwrap();
    s = Snapshot::read(&c.f);
    let first = get(&c, "/archive");
    let groups = first["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 100);
    assert_eq!(groups[0]["id"], "g-30");
    for (n, g) in groups.iter().enumerate().skip(1) {
        assert_eq!(g["id"], format!("extra-{n:03}"));
    }
    assert_eq!(first["next_group_offset"], 100);
    check(&c, &s, true, true);
    let last = get(&c, "/archive/groups/5/100");
    assert_eq!(last["groups"].as_array().unwrap().len(), 2);
    assert_eq!(last["groups"][0]["id"], "extra-100");
    assert_eq!(last["groups"][1]["id"], "g-10");
    assert_eq!(last["next_group_offset"], Value::Null);
    assert_eq!(last["items"], first["items"]);
    assert_eq!(last["total"], 2);
    check(&c, &s, true, true);
    assert_eq!(post("g-10", "restore", 5).status, 200);
    s = s.group_transition(&c.f, "g-10", true);
    check(&c, &s, true, true);
    rejected(
        c.request(
            Some(&c.pair),
            "GET",
            &format!("{}/archive/groups/5/100", board()),
            &Value::Null,
        ),
        409,
    );
    check(&c, &s, true, true);
}
