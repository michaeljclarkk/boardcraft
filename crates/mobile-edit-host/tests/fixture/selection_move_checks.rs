use crate::{
    create::{Case, login, rejected},
    fixture::Schema,
    reorder_state::{self, Snapshot},
    selection::{HIDDEN, SELECTED},
};
use boardcraft_board::{BoardId, ItemId, store};
use boardcraft_mobile_edit_host::Host;
use serde_json::{Value, json};
use uuid::Uuid;

fn route() -> String {
    format!("/preview/api/navigation/boards/{SELECTED}/items/move-selection")
}
fn root(n: u128) -> Value {
    json!({"id":Uuid::from_u128(n),"revision":1})
}
fn draft() -> Value {
    json!({"operation_id":Uuid::new_v4(),"group_id":"g-90","board_revision":1,"items":[root(67),root(59),root(61)]})
}
fn post(c: &Case, body: &Value) -> crate::browser_transport::Reply {
    c.request(Some(&c.pair), "POST", &route(), body)
}

pub fn access() {
    let c = Case::new();
    reorder_state::seed(&c.f, c.member);
    let mut db = c.f.connect();
    let original = Snapshot::read(&c.f);
    let body = draft();
    let unchanged = |baseline: &Snapshot, role: &str, joined, live| {
        assert_eq!(Snapshot::read(&c.f), *baseline);
        c.catalog.check_grants(&c.f, joined);
        assert_eq!(c.session.present(&c.f), live);
        assert_eq!(db_role(&c), role);
    };
    rejected(c.request(None, "POST", &route(), &body), 401);
    unchanged(&original, "member", true, true);
    db.execute(
        "UPDATE accounts SET role='viewer' WHERE id=$1",
        &[&c.member],
    )
    .unwrap();
    rejected(post(&c, &body), 403);
    unchanged(&original, "viewer", true, true);
    db.execute(
        "UPDATE accounts SET role='member' WHERE id=$1",
        &[&c.member],
    )
    .unwrap();
    c.catalog.membership(&c.f, false);
    rejected(post(&c, &body), 403);
    unchanged(&original, "member", false, true);
    c.catalog.membership(&c.f, true);
    for (k, v, code) in [
        ("operation_id", json!(Uuid::nil()), 422),
        ("items", json!([]), 422),
        ("items", json!([root(67), root(67)]), 422),
        (
            "items",
            json!((1..=10_001).map(root).collect::<Vec<_>>()),
            422,
        ),
        ("items", json!([root(67), root(88)]), 404),
        ("items", json!([root(67), root(62)]), 404),
        ("items", json!([root(67), root(57)]), 404),
        ("group_id", json!("g-30"), 422),
        ("group_id", json!("missing"), 422),
        ("group_id", json!("g-70"), 422),
        ("board_revision", json!(2), 409),
        (
            "items",
            json!([root(67),{"id":Uuid::from_u128(59),"revision":2}]),
            409,
        ),
        ("anchor_id", json!(Uuid::from_u128(73)), 400),
    ] {
        let mut bad = body.clone();
        bad[k] = v;
        rejected(post(&c, &bad), code);
        unchanged(&original, "member", true, true);
    }
    for (path, code) in [
        (
            route().replace(&SELECTED.to_string(), &HIDDEN.to_string()),
            403,
        ),
        // Native paths reject browser transport headers before route dispatch.
        (route().replace("/preview/api", ""), 403),
        (route().replace("/navigation/boards/", "/boards/"), 404),
        (
            route().replace(&SELECTED.to_string(), &c.f.board.to_string()),
            403,
        ),
    ] {
        rejected(c.request(Some(&c.pair), "POST", &path, &body), code);
        unchanged(&original, "member", true, true);
    }
    let nav =
        Host::start_with_navigation(&c.f.schema.config, BoardId::new(c.f.board).unwrap()).unwrap();
    for port in [c.host.port, c.f.host.port, nav.port] {
        let path = route();
        for route in [
            path.trim_start_matches("/preview/api"),
            &format!("/boards/{}/items/move-selection", c.f.board),
        ] {
            rejected(
                crate::browser_transport::send(
                    port,
                    "POST",
                    route,
                    &format!("127.0.0.1:{port}"),
                    &[],
                    &body.to_string(),
                ),
                404,
            );
            unchanged(&original, "member", true, true);
        }
    }
    // Exactly 500 rows before the move; emptying the second Work would add a 501st.
    db.execute("INSERT INTO board_groups SELECT $1,'extra-'||n,'Empty',4,true FROM generate_series(1,494) n",&[&SELECTED]).unwrap();
    let full = Snapshot::read(&c.f);
    rejected(post(&c, &body), 413);
    unchanged(&full, "member", true, true);
    c.session.revoke(&c.f);
    rejected(post(&c, &body), 401);
    unchanged(&full, "member", true, false);
}
fn db_role(c: &Case) -> String {
    c.f.connect()
        .query_one("SELECT role FROM accounts WHERE id=$1", &[&c.member])
        .unwrap()
        .get(0)
}

pub fn replay() {
    let c = Case::new();
    reorder_state::seed(&c.f, c.member);
    let original = Snapshot::read(&c.f);
    let second = Host::start_preview(&c.f.schema.config, BoardId::new(c.f.board).unwrap()).unwrap();
    let pair = login(second.port, "reader", &c.password);
    let a = draft();
    let b = draft();
    let port = c.host.port;
    let first = &c.pair;
    let barrier = std::sync::Barrier::new(2);
    let (left, right) = std::thread::scope(|s| {
        let left = s.spawn(|| {
            barrier.wait();
            Case::request_at(port, Some(first), "POST", &route(), &a)
        });
        barrier.wait();
        let right = Case::request_at(second.port, Some(&pair), "POST", &route(), &b);
        (left.join().unwrap(), right)
    });
    let (command, ack, loser, other, other_pair) = if left.status == 200 {
        (&a, left, right, second.port, &pair)
    } else {
        (&b, right, left, port, first)
    };
    assert_eq!(ack.status, 200);
    rejected(loser, 409);
    let accepted = original.batch(&c.f, c.member, "g-90", &[73, 77, 67, 59, 61], &[67, 59, 61]);
    let check = || {
        assert_eq!(Snapshot::read(&c.f), accepted);
        c.catalog.check_grants(&c.f, true);
        assert!(c.session.present(&c.f));
    };
    check();
    let replay = Case::request_at(other, Some(other_pair), "POST", &route(), command);
    assert_eq!(replay.status, 200);
    assert_eq!(replay.body, ack.body);
    let result: Value = serde_json::from_str(&ack.body).unwrap();
    let saved: Vec<Uuid> =
        c.f.connect()
            .query_one("SELECT affected_ids FROM group_reorder_receipts", &[])
            .unwrap()
            .get(0);
    assert_eq!(
        result,
        json!({"operation_id":command["operation_id"],"board_id":SELECTED,"affected_ids":saved})
    );
    check();
    for (k, v) in [
        ("items", json!([root(61), root(59), root(67)])),
        ("group_id", json!("g-10")),
        ("board_revision", json!(2)),
        (
            "items",
            json!([root(67),root(59),{"id":Uuid::from_u128(61),"revision":2}]),
        ),
    ] {
        let mut changed = command.clone();
        changed[k] = v;
        rejected(post(&c, &changed), 409);
        check();
    }
    let mut db = c.f.connect();
    let pw = Uuid::new_v4().to_string();
    let peer =
        c.f.seed_account("batch-peer", &pw, boardcraft_account::Role::Member);
    db.execute("INSERT INTO workspace_members SELECT workspace_id,$2 FROM board_policies WHERE board_id=$1", &[&SELECTED,&peer]).unwrap();
    db.execute(
        "INSERT INTO board_subscribers VALUES ($1,$2)",
        &[&SELECTED, &peer],
    )
    .unwrap();
    let peer_pair = login(second.port, "batch-peer", &pw);
    rejected(
        Case::request_at(second.port, Some(&peer_pair), "POST", &route(), command),
        409,
    );
    check();
    let anchor = json!({"operation_id":command["operation_id"],"group_id":"g-90","anchor_id":Uuid::from_u128(61),"side":"before","board_revision":2,"item_revision":2});
    rejected(
        c.request(
            Some(first),
            "POST",
            &format!(
                "/preview/api/navigation/boards/{SELECTED}/items/{}/reorder",
                Uuid::from_u128(67)
            ),
            &anchor,
        ),
        409,
    );
    check();
    db.execute(
        "UPDATE accounts SET role='viewer' WHERE id=$1",
        &[&c.member],
    )
    .unwrap();
    rejected(post(&c, command), 403);
    check();
    db.execute(
        "UPDATE accounts SET role='member' WHERE id=$1",
        &[&c.member],
    )
    .unwrap();
    let mut single = draft();
    single["board_revision"] = json!(2);
    single["items"] = json!([root(73)]);
    assert_eq!(post(&c, &single).status, 200);
    accepted.batch(&c.f, c.member, "g-90", &[77, 67, 59, 61, 73], &[73]);
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
        include_str!("../../../board/migrations/0006_group_reorder.sql"),
    ] {
        db.batch_execute(sql).unwrap();
    }
    db.batch_execute("CREATE TABLE rustday_migrations(version integer PRIMARY KEY); INSERT INTO rustday_migrations SELECT generate_series(1,6)").unwrap();
    db.execute("INSERT INTO boards VALUES ($1,2)", &[&SELECTED])
        .unwrap();
    db.execute(
        "INSERT INTO board_groups VALUES ($1,'work','Work',0,true)",
        &[&SELECTED],
    )
    .unwrap();
    for (n, date) in [
        (73, None),
        (77, Some("2000-01-01")),
        (61, Some("2000-01-01")),
        (67, Some("2000-02-01")),
    ] {
        db.execute("INSERT INTO board_items(id,board_id,group_id,title,position,created_at) VALUES ($1,$2,'work','Ink',10,$3::text::timestamptz)",&[&Uuid::from_u128(n),&SELECTED,&date]).unwrap();
    }
    let old = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let item = Uuid::from_u128(67);
    let anchor = Uuid::from_u128(61);
    db.execute(
        "INSERT INTO group_reorder_receipts VALUES ($1,$2,$3,$4,'work',$5,false,1,1)",
        &[&old, &actor, &SELECTED, &item, &anchor],
    )
    .unwrap();
    let board = BoardId::new(SELECTED).unwrap();
    let legacy = store::Reorder {
        operation: old,
        group: "work",
        anchor,
        after: false,
        board_revision: 1,
        item_revision: 1,
    };
    for _ in 0..2 {
        store::migrate(&mut db).unwrap();
        db = connect();
        let mut tx = db.transaction().unwrap();
        store::reorder_group_item(&mut tx, actor, board, ItemId::new(item).unwrap(), &legacy)
            .unwrap();
        tx.commit().unwrap();
    }
    let items = [(item, 1)];
    let mut batch = store::SelectionMove {
        operation: old,
        group: "work",
        board_revision: 2,
        items: &items,
    };
    let mut tx = db.transaction().unwrap();
    assert!(matches!(
        store::move_selected_items(&mut tx, actor, board, &batch),
        Err(store::ReorderError::Conflict)
    ));
    tx.rollback().unwrap();
    batch.operation = Uuid::new_v4();
    let mut tx = db.transaction().unwrap();
    store::move_selected_items(&mut tx, actor, board, &batch).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        store::load_items(&mut connect(), board)
            .unwrap()
            .unwrap()
            .iter()
            .map(|r| r.id.as_uuid().as_u128())
            .collect::<Vec<_>>(),
        [73, 61, 77, 67]
    );
}
