use crate::{
    create::{Case, rejected},
    lifecycle,
    reorder_state::Snapshot,
    selection::{HIDDEN, SELECTED},
};
use serde_json::{Value, json};
use uuid::Uuid;

fn board() -> String {
    format!("/web/api/navigation/boards/{SELECTED}")
}
fn page() -> String {
    format!("{}/archive", board())
}
fn route(id: u128, action: &str) -> String {
    format!("{}/items/{}/{action}", board(), Uuid::from_u128(id))
}
fn draft(b: i64, i: i64) -> Value {
    json!({"operation_id":Uuid::new_v4(),"board_revision":b,"item_revision":i})
}
fn role(c: &Case, name: &str) {
    c.f.connect()
        .execute(
            "UPDATE accounts SET role=$2 WHERE id=$1",
            &[&c.member, &name],
        )
        .unwrap();
}
fn inventory(c: &Case) -> Value {
    let r = c.request(Some(&c.pair), "GET", &page(), &Value::Null);
    assert_eq!(r.status, 200);
    serde_json::from_str(&r.body).unwrap()
}
fn owner() -> Case {
    let c = Case::new();
    lifecycle::seed(&c.f, c.member);
    role(&c, "owner");
    c
}

pub fn access() {
    let c = owner();
    let d = draft(1, 1);
    let s = Snapshot::read(&c.f);
    let endpoints = [
        ("GET", page()),
        ("POST", route(67, "archive")),
        ("POST", route(67, "restore")),
    ];
    let unchanged = |baseline: &Snapshot, joined, live| {
        assert_eq!(Snapshot::read(&c.f), *baseline);
        c.catalog.check_grants(&c.f, joined);
        assert_eq!(c.session.present(&c.f), live);
    };
    for (method, path) in &endpoints {
        rejected(c.request(None, method, path, &d), 401);
        unchanged(&s, true, true);
        rejected(
            c.request(
                Some(&c.pair),
                method,
                &path.replace(&SELECTED.to_string(), &HIDDEN.to_string()),
                &d,
            ),
            403,
        );
        unchanged(&s, true, true);
        rejected(
            c.request(
                Some(&c.pair),
                method,
                &path.replace("/navigation/boards/", "/boards/"),
                &d,
            ),
            404,
        );
        unchanged(&s, true, true);
        rejected(
            c.request(Some(&c.pair), method, &path.replace("/web/api", ""), &d),
            403,
        );
        unchanged(&s, true, true);
    }
    c.catalog.membership(&c.f, false);
    for (method, path) in &endpoints {
        rejected(c.request(Some(&c.pair), method, path, &d), 403);
        unchanged(&s, false, true);
    }
    c.catalog.membership(&c.f, true);
    for (id, action, body, status) in [
        (62, "archive", d.clone(), 404),
        (57, "restore", d.clone(), 404),
        (67, "restore", d.clone(), 422),
        (68, "restore", d.clone(), 422),
        (69, "archive", d.clone(), 422),
        (67, "archive", draft(2, 1), 409),
        (67, "archive", draft(1, 2), 409),
        (
            67,
            "archive",
            json!({"operation_id":Uuid::nil(),"board_revision":1,"item_revision":1}),
            422,
        ),
    ] {
        rejected(
            c.request(Some(&c.pair), "POST", &route(id, action), &body),
            status,
        );
        unchanged(&s, true, true);
    }
    assert_eq!(
        c.request(Some(&c.pair), "POST", &route(67, "archive"), &d)
            .status,
        200
    );
    let archived = s.lifecycle(&c.f, c.member, "archive");
    c.f.connect()
        .execute(
            "UPDATE board_policies SET creator_id=$1 WHERE board_id=$2",
            &[&c.member, &SELECTED],
        )
        .unwrap();
    for name in ["member", "viewer"] {
        role(&c, name);
        let p = inventory(&c);
        assert_eq!(p["total"], 2);
        assert_eq!(p["can_manage"], false);
        assert_eq!(p["items"][1]["restorable"], false);
        for (_, path) in &endpoints[1..] {
            rejected(c.request(Some(&c.pair), "POST", path, &d), 403);
            unchanged(&archived, true, true);
        }
    }
    c.session.revoke(&c.f);
    for (method, path) in &endpoints {
        rejected(c.request(Some(&c.pair), method, path, &d), 401);
        unchanged(&archived, true, false);
    }
}

pub fn replay() {
    let c = owner();
    let s = Snapshot::read(&c.f);
    let d = draft(1, 1);
    let post = |id, action, body| c.request(Some(&c.pair), "POST", &route(id, action), body);
    let other = boardcraft_mobile_edit_host::Host::start_web_api(
        &c.f.schema.config,
        boardcraft_board::BoardId::new(c.f.board).unwrap(),
    )
    .unwrap();
    let pair = crate::create::login(other.port, "reader", &c.password);
    let rival = draft(1, 1);
    let start = std::sync::Barrier::new(2);
    let (left, right) = std::thread::scope(|scope| {
        let fork = scope.spawn(|| {
            start.wait();
            Case::request_at(
                other.port,
                Some(&pair),
                "POST",
                &route(67, "archive"),
                &rival,
            )
        });
        start.wait();
        (post(67, "archive", &d), fork.join().unwrap())
    });
    let (cmd, ack, loser) = if left.status == 200 {
        (&d, left, right)
    } else {
        (&rival, right, left)
    };
    assert_eq!(ack.status, 200);
    rejected(loser, 409);
    let archived = s.lifecycle(&c.f, c.member, "archive");
    let check = || {
        assert_eq!(Snapshot::read(&c.f), archived);
        c.catalog.check_grants(&c.f, true);
        assert!(c.session.present(&c.f));
    };
    let replay = Case::request_at(other.port, Some(&pair), "POST", &route(67, "archive"), cmd);
    assert_eq!(replay.status, 200);
    assert_eq!(ack.body, replay.body);
    check();
    let p = inventory(&c);
    assert_eq!(p["board_revision"], 2);
    assert_eq!(p["total"], 2);
    assert_eq!(p["has_more"], false);
    let mut db = c.f.connect();
    let password = Uuid::new_v4().to_string();
    let peer =
        c.f.seed_account("recovery-peer", &password, boardcraft_account::Role::Owner);
    db.execute("INSERT INTO workspace_members SELECT workspace_id,$2 FROM board_policies WHERE board_id=$1", &[&SELECTED,&peer]).unwrap();
    db.execute(
        "INSERT INTO board_subscribers VALUES ($1,$2)",
        &[&SELECTED, &peer],
    )
    .unwrap();
    let peer_pair = crate::create::login(other.port, "recovery-peer", &password);
    rejected(
        Case::request_at(
            other.port,
            Some(&peer_pair),
            "POST",
            &route(67, "archive"),
            cmd,
        ),
        409,
    );
    check();
    // A direct bearer rename has its own configured target, but the same active-row predicate.
    let direct = boardcraft_mobile_edit_host::Host::start(
        &c.f.schema.config,
        boardcraft_board::BoardId::new(SELECTED).unwrap(),
    )
    .unwrap();
    let token = boardcraft_account::sign_in(&c.f.schema.config, "reader", &c.password).unwrap();
    rejected(
        crate::browser_transport::send(
            direct.port,
            "PUT",
            &format!("/boards/{SELECTED}/items/{}/name", Uuid::from_u128(67)),
            &format!("127.0.0.1:{}", direct.port),
            &[("Authorization", &format!("Bearer {}", token.encoded()))],
            &json!({"title":"No","revision":2}).to_string(),
        ),
        404,
    );
    check();
    for (i, (id, name, group, state, can)) in [
        (57, "Stored", "Retired", 0, false),
        (67, "Ink", "Work", 1, true),
    ]
    .into_iter()
    .enumerate()
    {
        let r = &p["items"][i];
        assert_eq!(r["id"], json!(Uuid::from_u128(id)));
        assert_eq!(r["state"], state);
        assert_eq!(r["effective_state"], 1);
        assert_eq!(r["title"], name);
        assert_eq!(r["group_name"], group);
        assert_eq!(r["restorable"], can);
    }
    for (id, action) in [(67, "restore"), (61, "archive")] {
        rejected(post(id, action, cmd), 409);
        check();
    }
    let mut changed = cmd.clone();
    changed["board_revision"] = json!(2);
    rejected(post(67, "archive", &changed), 409);
    check();
    let reorder = json!({"operation_id":Uuid::new_v4(),"board_revision":2,"item_revision":2,"group_id":"g-90","anchor_id":Uuid::from_u128(61),"side":"after"});
    let mut anchor = reorder.clone();
    anchor["anchor_id"] = json!(Uuid::from_u128(67));
    anchor["item_revision"] = json!(1);
    for (method, path, body, code) in [
        (
            "PUT",
            route(67, "name"),
            json!({"title":"No","revision":2}),
            404,
        ),
        (
            "PUT",
            route(67, "group"),
            json!({"group_id":"g-10","expected_revision":2}),
            404,
        ),
        ("POST", route(67, "reorder"), reorder, 404),
        ("POST", route(61, "reorder"), anchor, 409),
        (
            "POST",
            format!("{}/items/move-selection", board()),
            json!({"operation_id":Uuid::new_v4(),"board_revision":2,"group_id":"g-10","items":[{"id":Uuid::from_u128(67),"revision":2}]}),
            404,
        ),
    ] {
        rejected(c.request(Some(&c.pair), method, &path, &body), code);
        check();
    }
    let restore = draft(2, 2);
    // Restoring one more active root must not overflow the grid projection.
    db.execute("INSERT INTO board_groups(board_id,id,name,position,active) SELECT $1,'empty-'||n,'Empty',4,true FROM generate_series(1,495) n", &[&SELECTED]).unwrap();
    let full = Snapshot::read(&c.f);
    rejected(post(67, "restore", &restore), 413);
    assert_eq!(Snapshot::read(&c.f), full);
    c.catalog.check_grants(&c.f, true);
    assert!(c.session.present(&c.f));
    db.execute(
        "DELETE FROM board_groups WHERE board_id=$1 AND id LIKE 'empty-%'",
        &[&SELECTED],
    )
    .unwrap();
    check();
    // Deleted ancestors outrank archived children. Also check a truly bounded first page.
    db.execute(
        "UPDATE board_groups SET deleted=true WHERE board_id=$1 AND id='g-30'",
        &[&SELECTED],
    )
    .unwrap();
    db.execute(
        "UPDATE board_items SET state=1 WHERE id=$1",
        &[&Uuid::from_u128(57)],
    )
    .unwrap();
    assert_eq!(inventory(&c)["total"], 1);
    db.execute(
        "UPDATE board_items SET state=0 WHERE id=$1",
        &[&Uuid::from_u128(57)],
    )
    .unwrap();
    db.execute(
        "UPDATE board_groups SET deleted=false WHERE board_id=$1 AND id='g-30'",
        &[&SELECTED],
    )
    .unwrap();
    check();
    let filler: Vec<_> = (1000..1100).map(Uuid::from_u128).collect();
    db.execute("INSERT INTO board_items(id,board_id,group_id,title,position,state) SELECT id,$1,'g-90','Recovery',1,1 FROM unnest($2::uuid[]) id", &[&SELECTED,&filler]).unwrap();
    let extra = Snapshot::read(&c.f);
    let many = inventory(&c);
    assert_eq!(many["total"], 102);
    assert_eq!(many["has_more"], true);
    assert_eq!(many["items"].as_array().unwrap().len(), 100);
    assert_eq!(many["items"][99]["id"], json!(filler[98]));
    rejected(post(67, "restore", &restore), 404);
    assert_eq!(Snapshot::read(&c.f), extra);
    c.catalog.check_grants(&c.f, true);
    assert!(c.session.present(&c.f));
    db.execute("DELETE FROM board_items WHERE id=ANY($1)", &[&filler])
        .unwrap();
    check();
    assert_eq!(post(67, "restore", &restore).status, 200);
    archived.lifecycle(&c.f, c.member, "restore");
    assert_eq!(inventory(&c)["total"], 1);
    // Empty an active group and verify that the left join retains it.
    assert_eq!(post(59, "archive", &draft(3, 1)).status, 200);
    let groups: Value =
        serde_json::from_str(&c.request(Some(&c.pair), "GET", &board(), &Value::Null).body)
            .unwrap();
    assert_eq!(groups[2]["id"], "g-20");
    assert_eq!(groups[2]["items"], json!([]));
}

pub fn migration() {
    use crate::fixture::{Schema, seed_account_on};
    use boardcraft_board::{BoardId, store};
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
        include_str!("../../../board/migrations/0007_selection_move.sql"),
        include_str!("../../../account/migrations/0001_account_session.sql"),
    ] {
        db.batch_execute(sql).unwrap();
    }
    db.batch_execute("CREATE TABLE rustday_migrations(version integer PRIMARY KEY);INSERT INTO rustday_migrations SELECT generate_series(1,7)").unwrap();
    let password = Uuid::new_v4().to_string();
    let actor = seed_account_on(
        &mut db,
        "upgrade",
        &password,
        boardcraft_account::Role::Member,
    );
    let token = boardcraft_account::sign_in(&schema.config, "upgrade", &password).unwrap();
    db.execute("INSERT INTO boards VALUES ($1,3)", &[&SELECTED])
        .unwrap();
    db.execute(
        "INSERT INTO board_groups VALUES ($1,'g','Work',0,true)",
        &[&SELECTED],
    )
    .unwrap();
    let old = Uuid::new_v4();
    let item = Uuid::from_u128(67);
    let anchor = Uuid::from_u128(61);
    for id in [item, anchor] {
        db.execute("INSERT INTO board_items(id,board_id,group_id,title,position,created_at,created_by,creation_group_id,creation_title) VALUES ($1,$2,'g','Ink',30,'2001-01-01',$3,'g','Ink')", &[&id,&SELECTED,&actor]).unwrap();
    }
    db.execute("INSERT INTO group_reorder_receipts(operation_id,actor_id,board_id,group_id,board_revision,selected_ids,selected_revisions,affected_ids) VALUES ($1,$2,$3,'g',2,ARRAY[$4::uuid],ARRAY[1],ARRAY[$4::uuid])", &[&old,&actor,&SELECTED,&item]).unwrap();
    let legacy = |db: &mut postgres::Client| {
        db.query(
            "SELECT (to_jsonb(i)-'state')::text FROM board_items i ORDER BY position,created_at,id",
            &[],
        )
        .unwrap()
        .into_iter()
        .map(|r| r.get::<_, String>(0))
        .collect::<Vec<_>>()
    };
    let before = legacy(&mut db);
    for _ in 0..2 {
        store::migrate(&mut db).unwrap();
        boardcraft_account::migrate(&schema.config).unwrap();
        db = connect();
        assert_eq!(before, legacy(&mut db));
        let account = boardcraft_account::resolve(&schema.config, &token).unwrap();
        assert_eq!(account.id, actor);
        assert_eq!(account.role, boardcraft_account::Role::Member);
        let board = BoardId::new(SELECTED).unwrap();
        let mut tx = db.transaction().unwrap();
        assert_eq!(
            store::move_selected_items(
                &mut tx,
                actor,
                board,
                &store::SelectionMove {
                    operation: old,
                    group: "g",
                    board_revision: 2,
                    items: &[(item, 1)]
                }
            )
            .unwrap(),
            [item]
        );
        assert!(matches!(
            store::transition_root(
                &mut tx,
                actor,
                board,
                boardcraft_board::ItemId::new(item).unwrap(),
                &store::RootCommand {
                    action: store::Lifecycle::Archive,
                    operation: old,
                    board_revision: 3,
                    item_revision: 1
                }
            ),
            Err(store::ReorderError::Conflict)
        ));
        tx.rollback().unwrap();
        assert_eq!(
            store::load_items(&mut db, board)
                .unwrap()
                .unwrap()
                .into_iter()
                .map(|r| r.id.as_uuid())
                .collect::<Vec<_>>(),
            [anchor, item]
        );
    }
}
