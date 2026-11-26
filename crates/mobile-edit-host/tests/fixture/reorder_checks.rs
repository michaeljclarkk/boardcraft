use crate::{
    browser_transport::send,
    create::{Case, login, rejected},
    fixture::Schema,
    reorder_state::{self, Snapshot},
    selection::SELECTED,
};
use boardcraft_board::{BoardId, ItemId, store};
use boardcraft_mobile_edit_host::Host;
use serde_json::{Value, json};
use uuid::Uuid;

fn path(item: u128) -> String {
    format!(
        "/preview/api/navigation/boards/{SELECTED}/items/{}/reorder",
        Uuid::from_u128(item)
    )
}
fn draft() -> Value {
    json!({"operation_id":Uuid::new_v4(),"group_id":"g-90","anchor_id":Uuid::from_u128(61),"side":"before","board_revision":1,"item_revision":1})
}

pub fn access() {
    let c = Case::new();
    reorder_state::seed(&c.f, c.member);
    let original = Snapshot::read(&c.f);
    let body = draft();
    let post = |item, b: &Value| c.request(Some(&c.pair), "POST", &path(item), b);
    let unchanged = || original.check(&c.f, c.member, 0);
    rejected(c.request(None, "POST", &path(67), &body), 401);
    unchanged();
    let mut db = c.f.connect();
    db.execute(
        "UPDATE accounts SET role='viewer' WHERE id=$1",
        &[&c.member],
    )
    .unwrap();
    rejected(post(67, &body), 403);
    unchanged();
    db.execute(
        "UPDATE accounts SET role='member' WHERE id=$1",
        &[&c.member],
    )
    .unwrap();
    c.catalog.membership(&c.f, false);
    rejected(post(67, &body), 403);
    unchanged();
    c.catalog.check_grants(&c.f, false);
    c.catalog.membership(&c.f, true);
    for n in [88, 57, 59, 62, 17] {
        rejected(post(n, &body), 404);
        unchanged();
    }
    for (key, value, code) in [
        ("group_id", json!("missing"), 422),
        ("group_id", json!("g-30"), 422),
        ("group_id", json!("g-70"), 422),
        ("anchor_id", json!(Uuid::from_u128(88)), 409),
        ("anchor_id", json!(Uuid::from_u128(67)), 422),
        ("anchor_id", json!(Uuid::from_u128(59)), 409),
        ("anchor_id", json!(Uuid::from_u128(62)), 409),
        ("side", json!("first"), 400),
        ("position", json!(0), 400),
        ("item_revision", json!(0), 409),
        ("board_revision", json!(0), 409),
    ] {
        let mut bad = body.clone();
        bad[key] = value;
        rejected(post(67, &bad), code);
        unchanged();
    }
    let nav =
        Host::start_with_navigation(&c.f.schema.config, BoardId::new(c.f.board).unwrap()).unwrap();
    for port in [nav.port, c.f.host.port, c.host.port] {
        let full = path(67);
        let scoped = format!(
            "/boards/{}/items/{}/reorder",
            c.f.board,
            Uuid::from_u128(17)
        );
        for route in [full.strip_prefix("/preview/api").unwrap(), &scoped, &full] {
            if port == c.host.port && route == full {
                continue;
            }
            rejected(
                send(
                    port,
                    "POST",
                    route,
                    &format!("127.0.0.1:{port}"),
                    &[],
                    &body.to_string(),
                ),
                404,
            );
            unchanged();
        }
    }
    // The anchor was present at read time but a group move does not advance the board revision.
    assert_eq!(
        c.request(
            Some(&c.pair),
            "GET",
            &format!("/preview/api/navigation/boards/{SELECTED}"),
            &Value::Null
        )
        .status,
        200
    );
    let moved = c.request(
        Some(&c.pair),
        "PUT",
        &path(61).replace("/reorder", "/group"),
        &json!({"group_id":"g-20","expected_revision":1}),
    );
    assert_eq!(moved.status, 200);
    let baseline = Snapshot::read(&c.f);
    rejected(post(67, &body), 409);
    assert_eq!(Snapshot::read(&c.f), baseline);
    assert!(c.session.present(&c.f));
    c.session.revoke(&c.f);
    rejected(post(67, &body), 401);
    assert_eq!(Snapshot::read(&c.f), baseline);
    assert!(!c.session.present(&c.f));
}

pub fn revision_and_replay() {
    let c = Case::new();
    reorder_state::seed(&c.f, c.member);
    let initial = Snapshot::read(&c.f);
    let second = Host::start_preview(&c.f.schema.config, BoardId::new(c.f.board).unwrap()).unwrap();
    let pair = login(second.port, "reader", &c.password);
    // Separate host connections compete at the database, not in one HTTP queue.
    let route = path(67);
    let a = draft();
    let b = draft();
    let port = c.host.port;
    let first = &c.pair;
    let barrier = std::sync::Barrier::new(2);
    let (left, right) = std::thread::scope(|s| {
        let left = s.spawn(|| {
            barrier.wait();
            Case::request_at(port, Some(first), "POST", &route, &a)
        });
        barrier.wait();
        let right = Case::request_at(second.port, Some(&pair), "POST", &route, &b);
        (left.join().unwrap(), right)
    });
    let (command, ack, loser) = if left.status == 200 {
        (&a, left, right)
    } else {
        (&b, right, left)
    };
    assert_eq!(ack.status, 200);
    rejected(loser, 409);
    initial.check(&c.f, c.member, 1);
    let post = |body: &Value| c.request(Some(&c.pair), "POST", &route, body);
    assert_eq!(
        serde_json::from_str::<Value>(&ack.body).unwrap(),
        json!({"operation_id":command["operation_id"],"board_id":SELECTED,"item_id":Uuid::from_u128(67)})
    );
    let replay = Case::request_at(
        if command == &a { second.port } else { port },
        Some(if command == &a { &pair } else { first }),
        "POST",
        &route,
        command,
    );
    assert_eq!(replay.status, 200);
    assert_eq!(replay.body, ack.body);
    initial.check(&c.f, c.member, 1);
    for (key, v) in [
        ("side", json!("after")),
        ("group_id", json!("g-20")),
        ("board_revision", json!(2)),
        ("item_revision", json!(2)),
        ("anchor_id", json!(Uuid::from_u128(77))),
    ] {
        let mut changed = command.clone();
        changed[key] = v;
        rejected(post(&changed), 409);
        initial.check(&c.f, c.member, 1);
    }
    let mut db = c.f.connect();
    let pw = Uuid::new_v4().to_string();
    let peer =
        c.f.seed_account("peer", &pw, boardcraft_account::Role::Member);
    db.execute("INSERT INTO workspace_members SELECT workspace_id,$2 FROM board_policies WHERE board_id=$1", &[&SELECTED,&peer]).unwrap();
    db.execute(
        "INSERT INTO board_subscribers VALUES ($1,$2)",
        &[&SELECTED, &peer],
    )
    .unwrap();
    let other = login(second.port, "peer", &pw);
    rejected(
        Case::request_at(second.port, Some(&other), "POST", &route, command),
        409,
    );
    initial.check(&c.f, c.member, 1);
    let edit = |item, field, data| {
        c.request(
            Some(&c.pair),
            "PUT",
            &path(item).replace("reorder", field),
            &data,
        )
    };
    rejected(
        edit(
            67,
            "group",
            json!({"group_id":"g-20","expected_revision":1}),
        ),
        409,
    );
    initial.check(&c.f, c.member, 1);
    assert_eq!(
        edit(61, "name", json!({"revision":1,"title":"Sibling"})).status,
        200
    );
    assert_eq!(
        edit(67, "name", json!({"revision":2,"title":"Selected"})).status,
        200
    );
    let baseline = Snapshot::read(&c.f);
    let mut stale = draft();
    stale["board_revision"] = json!(2);
    stale["item_revision"] = json!(2);
    rejected(post(&stale), 409);
    assert_eq!(Snapshot::read(&c.f), baseline);
    stale["board_revision"] = json!(1);
    stale["item_revision"] = json!(3);
    rejected(post(&stale), 409);
    assert_eq!(Snapshot::read(&c.f), baseline);
    // Replays still require current edit access, even for the issuing actor.
    db.execute(
        "UPDATE accounts SET role='viewer' WHERE id=$1",
        &[&c.member],
    )
    .unwrap();
    rejected(post(command), 403);
    assert_eq!(Snapshot::read(&c.f), baseline);
}

pub fn migration() {
    let schema = Schema::new();
    let connect = || schema.config.connect(postgres::NoTls).unwrap();
    let mut db = connect();
    for sql in [
        include_str!("../../../board/migrations/0001_boards.sql"),
        include_str!("../../../board/migrations/0002_item_revision.sql"),
        include_str!("../../../board/migrations/0003_board_groups.sql"),
        include_str!("../../../board/migrations/0004_item_creation.sql"),
        include_str!("../../../board/migrations/0005_item_group_move.sql"),
    ] {
        db.batch_execute(sql).unwrap();
    }
    db.batch_execute("CREATE TABLE rustday_migrations(version integer PRIMARY KEY);INSERT INTO rustday_migrations SELECT generate_series(1,5)").unwrap();
    db.execute("INSERT INTO boards VALUES ($1)", &[&SELECTED])
        .unwrap();
    db.execute(
        "INSERT INTO board_groups VALUES ($1,'g','Work',0,true)",
        &[&SELECTED],
    )
    .unwrap();
    let actor = Uuid::new_v4();
    for (id, date) in [
        (73, None),
        (61, Some("2001-01-01")),
        (67, Some("2001-02-01")),
        (77, Some("2001-02-01")),
    ] {
        db.execute("INSERT INTO board_items(id,board_id,group_id,title,position,created_at,created_by,creation_title,creation_group_id,revision,group_revision) VALUES ($1,$2,'g','Ink',10,$3::text::timestamptz,$4,'Original','g',4,3)", &[&Uuid::from_u128(id),&SELECTED,&date,&actor]).unwrap();
    }
    let rows = |db: &mut postgres::Client| {
        db.query("SELECT (to_jsonb(i)-'position_revision'-'state')::text FROM board_items i ORDER BY position,created_at NULLS FIRST,id",&[]).unwrap().into_iter().map(|r|r.get::<_,String>(0)).collect::<Vec<_>>()
    };
    let before = rows(&mut db);
    for _ in 0..2 {
        store::migrate(&mut db).unwrap();
        db = connect();
        assert_eq!(rows(&mut db), before);
        assert!(
            db.query_one("SELECT bool_and(state=0) FROM board_items", &[])
                .unwrap()
                .get::<_, bool>(0)
        );
    }
    let mut tx = db.transaction().unwrap();
    let board = BoardId::new(SELECTED).unwrap();
    store::reorder_group_item(
        &mut tx,
        actor,
        board,
        ItemId::new(Uuid::from_u128(67)).unwrap(),
        &store::Reorder {
            operation: Uuid::new_v4(),
            group: "g",
            anchor: Uuid::from_u128(61),
            after: false,
            board_revision: 1,
            item_revision: 4,
        },
    )
    .unwrap();
    tx.commit().unwrap();
    assert_eq!(
        store::load_items(&mut connect(), board)
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|r| r.id.as_uuid().as_u128())
            .collect::<Vec<_>>(),
        [73, 67, 61, 77]
    );
}
