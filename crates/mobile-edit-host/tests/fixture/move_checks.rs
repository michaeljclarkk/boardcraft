use crate::{
    browser_transport::send,
    create::{Case, rejected},
    fixture,
    move_state::MoveState,
    selection::SELECTED,
};
use boardcraft_board::{BoardId, store};
use boardcraft_mobile_edit_host::Host;
use serde_json::{Value, json};
use uuid::Uuid;

fn path(id: u128) -> String {
    format!(
        "/web/api/navigation/boards/{SELECTED}/items/{}/group",
        Uuid::from_u128(id)
    )
}
fn draft(to: &str, rev: i64) -> Value {
    json!({"group_id":to,"expected_revision":rev})
}

pub fn access() {
    let c = Case::new();
    let state = MoveState::seed(&c.f, c.member);
    let baseline = |joined, live, archived| {
        state.check(&c.f, "g-90", "Ink", 1, 1);
        state.groups(&c.f, archived);
        c.catalog.check_grants(&c.f, joined);
        assert!(c.session.present(&c.f) == live);
    };
    let put = |id, to| c.request(Some(&c.pair), "PUT", &path(id), &draft(to, 1));
    rejected(c.request(None, "PUT", &path(61), &draft("g-20", 1)), 401);
    baseline(true, true, false);
    let mut db = c.f.connect();
    db.execute(
        "UPDATE accounts SET role='viewer' WHERE id=$1",
        &[&c.member],
    )
    .unwrap();
    rejected(put(61, "g-20"), 403);
    baseline(true, true, false);
    db.execute(
        "UPDATE accounts SET role='member' WHERE id=$1",
        &[&c.member],
    )
    .unwrap();
    c.catalog.membership(&c.f, false);
    rejected(put(61, "g-20"), 403);
    baseline(false, true, false);
    c.catalog.membership(&c.f, true);
    for group in ["missing", "g-30", "g-70"] {
        rejected(put(61, group), 422);
        baseline(true, true, false);
    }
    for id in [57, 62, 17, 88] {
        rejected(put(id, "g-20"), 404);
        baseline(true, true, false);
    }
    assert_eq!(
        c.request(
            Some(&c.pair),
            "GET",
            &format!("/web/api/navigation/boards/{SELECTED}"),
            &Value::Null
        )
        .status,
        200
    );
    db.execute(
        "UPDATE board_groups SET active=false WHERE board_id=$1 AND id='g-20'",
        &[&SELECTED],
    )
    .unwrap();
    rejected(put(61, "g-20"), 422);
    baseline(true, true, true);
    db.execute(
        "UPDATE board_groups SET active=true WHERE board_id=$1 AND id='g-20'",
        &[&SELECTED],
    )
    .unwrap();
    let nav =
        Host::start_with_navigation(&c.f.schema.config, BoardId::new(c.f.board).unwrap()).unwrap();
    for port in [nav.port, c.f.host.port, c.host.port] {
        let full = path(61);
        let native = full.strip_prefix("/web/api").unwrap();
        let scoped = format!("/boards/{}/items/{}/group", c.f.board, Uuid::from_u128(17));
        for route in [native, &scoped, &full] {
            if port == c.host.port && route == full {
                continue;
            }
            rejected(
                send(
                    port,
                    "PUT",
                    route,
                    &format!("127.0.0.1:{port}"),
                    &[],
                    &draft("g-20", 1).to_string(),
                ),
                404,
            );
            baseline(true, true, false);
        }
    }
    c.session.revoke(&c.f);
    rejected(put(61, "g-20"), 401);
    baseline(true, false, false);
}

pub fn revision_and_order() {
    let c = Case::new();
    let state = MoveState::seed(&c.f, c.member);
    let put = |id, to, rev| c.request(Some(&c.pair), "PUT", &path(id), &draft(to, rev));
    let check = |group, title, rev, stamp| {
        state.check(&c.f, group, title, rev, stamp);
        state.groups(&c.f, false);
        c.catalog.check_grants(&c.f, true);
        assert!(c.session.present(&c.f));
    };
    let rename = path(61).replace("/group", "/name");
    assert_eq!(
        c.request(
            Some(&c.pair),
            "PUT",
            &rename,
            &json!({"title":"Coral","revision":1})
        )
        .status,
        200
    );
    check("g-90", "Coral", 2, 1);
    for (to, base, rev, stamp) in [("g-20", 1, 3, 3), ("g-20", 3, 3, 3), ("g-90", 3, 4, 4)] {
        let reply = put(61, to, base);
        assert_eq!(reply.status, 200);
        assert_eq!(
            serde_json::from_str::<Value>(&reply.body).unwrap(),
            json!({"id":Uuid::from_u128(61),"group_id":to,"title":"Coral","revision":rev})
        );
        check(to, "Coral", rev, stamp);
    }
    for (id, to, rev, status) in [
        (61, "g-90", 1, 409),
        (61, "g-20", 3, 409),
        (61, "g-90", 5, 409),
        (61, "g-20", 0, 409),
        (88, "g-20", 0, 404),
    ] {
        rejected(put(id, to, rev), status);
        check("g-90", "Coral", 4, 4);
    }
    for extra in ["actor", "position", "title", "board_id"] {
        let mut body = draft("g-20", 4);
        body[extra] = json!(1);
        rejected(c.request(Some(&c.pair), "PUT", &path(61), &body), 400);
        check("g-90", "Coral", 4, 4);
    }
    assert_eq!(put(61, "g-20", 4).status, 200);
    check("g-20", "Coral", 5, 5);
    let order = |expected: &[u128]| {
        let reply = c.request(
            Some(&c.pair),
            "GET",
            &format!("/web/api/navigation/boards/{SELECTED}"),
            &Value::Null,
        );
        assert_eq!(reply.status, 200);
        let groups: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(
            groups[2]["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| Uuid::parse_str(r["id"].as_str().unwrap())
                    .unwrap()
                    .as_u128())
                .collect::<Vec<_>>(),
            expected
        );
        let flat = store::load_items(&mut c.f.connect(), BoardId::new(SELECTED).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(
            flat.into_iter()
                .map(|r| r.id.as_uuid().as_u128())
                .collect::<Vec<_>>(),
            [&[73][..], expected].concat()
        );
    };
    order(&[61, 59]);
    // Retransmission is scoped to the original creation, not the new group.
    let create_path = format!("/web/api/navigation/boards/{SELECTED}/items");
    let replay = |group: &str| {
        c.request(
            Some(&c.pair),
            "POST",
            &create_path,
            &json!({"id":Uuid::from_u128(61),"group_id":group,"title":"Ink"}),
        )
    };
    rejected(replay("g-20"), 409);
    check("g-20", "Coral", 5, 5);
    let original = replay("g-90");
    assert_eq!(original.status, 200);
    assert_eq!(
        serde_json::from_str::<Value>(&original.body).unwrap(),
        json!({"id":Uuid::from_u128(61),"title":"Coral","revision":5,"group_id":"g-20"})
    );
    check("g-20", "Coral", 5, 5);
    // Equalize the synthetic dates: the final key must now be the UUID.
    c.f.connect().execute("UPDATE board_items SET created_at=(SELECT created_at FROM board_items WHERE id=$1) WHERE id=$2", &[&Uuid::from_u128(61),&Uuid::from_u128(59)]).unwrap();
    order(&[59, 61]);
    let append = c.request(
        Some(&c.pair),
        "POST",
        &format!("/web/api/navigation/boards/{SELECTED}/items"),
        &json!({"id":Uuid::from_u128(80),"group_id":"g-20","title":"New"}),
    );
    assert_eq!(append.status, 200);
    assert_eq!(
        c.f.connect()
            .query_one(
                "SELECT position FROM board_items WHERE id=$1",
                &[&Uuid::from_u128(80)]
            )
            .unwrap()
            .get::<_, i64>(0),
        3
    );
    order(&[59, 61, 80]);
}

pub fn migration() {
    let schema = fixture::Schema::new();
    let connect = || schema.config.connect(postgres::NoTls).unwrap();
    let mut db = connect();
    for sql in [
        include_str!("../../../board/migrations/0001_boards.sql"),
        include_str!("../../../board/migrations/0002_item_revision.sql"),
        include_str!("../../../board/migrations/0003_board_groups.sql"),
        include_str!("../../../board/migrations/0004_item_creation.sql"),
    ] {
        db.batch_execute(sql).unwrap();
    }
    db.batch_execute("CREATE TABLE rustday_migrations(version integer PRIMARY KEY); INSERT INTO rustday_migrations VALUES (1),(2),(3),(4)").unwrap();
    db.execute("INSERT INTO boards VALUES ($1)", &[&SELECTED])
        .unwrap();
    db.execute(
        "INSERT INTO board_groups VALUES ($1,'g','Work',0,true)",
        &[&SELECTED],
    )
    .unwrap();
    let creator = Uuid::new_v4();
    db.execute("INSERT INTO board_items (id,board_id,position,group_id,title,revision,created_by,creation_title) VALUES ($1,$3,2,'g',' Padded ',6,$4,'Original'),($2,$3,1,'g','Ink',1,NULL,NULL)", &[&Uuid::from_u128(59),&Uuid::from_u128(61),&SELECTED,&creator]).unwrap();
    for _ in 0..2 {
        store::migrate(&mut db).unwrap();
        db = connect();
        let rows = db.query("SELECT id,title,revision,group_revision,created_at IS NULL,created_by,creation_group_id,creation_title FROM board_items WHERE board_id=$1 AND group_id='g' ORDER BY position", &[&SELECTED]).unwrap();
        assert_eq!(rows.len(), 2);
        for (r, (id, title, rev, by, group, original)) in rows.iter().zip([
            (61, "Ink", 1, None, None, None),
            (
                59,
                " Padded ",
                6,
                Some(creator),
                Some("g"),
                Some("Original"),
            ),
        ]) {
            assert_eq!(
                (
                    r.get::<_, Uuid>(0).as_u128(),
                    r.get::<_, &str>(1),
                    r.get::<_, i64>(2),
                    r.get::<_, i64>(3),
                    r.get::<_, bool>(4),
                    r.get::<_, Option<Uuid>>(5),
                    r.get::<_, Option<&str>>(6),
                    r.get::<_, Option<&str>>(7)
                ),
                (id, title, rev, 1, true, by, group, original)
            );
        }
    }
}
