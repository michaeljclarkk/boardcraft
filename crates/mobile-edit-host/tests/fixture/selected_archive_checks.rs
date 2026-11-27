//! Native bearer and matched preview requests share one atomic lifecycle owner.
use crate::{
    browser_transport::{Reply, send},
    create::Case,
    lifecycle,
    reorder_state::Snapshot,
    selection::{HIDDEN, SELECTED},
};
use boardcraft_board::{BoardId, ItemId, store};
use boardcraft_mobile_edit_host::Host;
use serde_json::{Value, json};
use uuid::Uuid;

fn draft(operation: Uuid, board: i64, items: &[(u128, i64)]) -> Value {
    json!({"operation_id":operation,"board_revision":board,"items":items.iter().map(|(id,revision)|json!({"id":Uuid::from_u128(*id),"revision":revision})).collect::<Vec<_>>()})
}
pub fn atomic_and_authorized() {
    let c = Case::new();
    lifecycle::seed(&c.f, c.member);
    let role = |role: &str| {
        c.f.connect()
            .execute(
                "UPDATE accounts SET role=$2 WHERE id=$1",
                &[&c.member, &role],
            )
            .unwrap();
    };
    role("owner");
    let native =
        Host::start_with_navigation(&c.f.schema.config, BoardId::new(c.f.board).unwrap()).unwrap();
    let token = boardcraft_account::sign_in(&c.f.schema.config, "reader", &c.password).unwrap();
    let route = |board, native| {
        format!(
            "{}{board}/{}/items/archive-selection",
            if native {
                "/navigation/boards/"
            } else {
                "/preview/api/navigation/boards/"
            },
            if native { "main-table" } else { "" }
        )
        .replace("//items", "/items")
    };
    let request = |is_native: bool, board, body: &Value| -> Reply {
        let path = route(board, is_native);
        if is_native {
            send(
                native.port,
                "POST",
                &path,
                &format!("127.0.0.1:{}", native.port),
                &[("Authorization", &format!("Bearer {}", token.encoded()))],
                &body.to_string(),
            )
        } else {
            c.request(Some(&c.pair), "POST", &path, body)
        }
    };
    let op = Uuid::new_v4();
    let valid = draft(op, 1, &[(67, 1), (59, 1), (61, 1)]);
    let initial = Snapshot::read(&c.f);
    let unchanged = |s: &Snapshot| {
        assert_eq!(Snapshot::read(&c.f), *s);
    };
    for native in [false, true] {
        assert_eq!(request(native, HIDDEN, &valid).status, 403);
        unchanged(&initial);
        let mut invalids = vec![
            (draft(op, 1, &[]), 422),
            (draft(op, 1, &[(67, 1), (67, 1)]), 422),
            (draft(Uuid::nil(), 1, &[(67, 1)]), 422),
            (draft(op, 1, &[(67, 1), (0, 1)]), 422),
            (draft(op, 1, &[(67, 1), (59, 0)]), 422),
            (draft(op, 2, &[(67, 1), (59, 1)]), 409),
            (draft(op, 1, &[(67, 1), (59, 2)]), 409),
            (draft(op, 1, &[(67, 1), (62, 1)]), 404),
            (draft(op, 1, &[(67, 1), (57, 1)]), 404),
            (draft(op, 1, &[(67, 1), (999, 1)]), 404),
            (draft(op, 1, &[(67, 1), (68, 1)]), 422),
            (draft(op, 1, &[(67, 1), (69, 1)]), 422),
            (
                draft(op, 1, &(1000..11000).map(|n| (n, 1)).collect::<Vec<_>>()),
                404,
            ),
            (
                draft(op, 1, &(1000..11001).map(|n| (n, 1)).collect::<Vec<_>>()),
                422,
            ),
        ];
        let mut extra = valid.clone();
        extra["actor"] = json!(c.member);
        invalids.push((extra, 400));
        for (body, status) in invalids {
            assert_eq!(request(native, SELECTED, &body).status, status);
            unchanged(&initial);
        }
    }
    for name in ["member", "viewer"] {
        role(name);
        for native in [false, true] {
            assert_eq!(request(native, SELECTED, &valid).status, 403);
            unchanged(&initial);
        }
    }
    role("owner");
    c.catalog.membership(&c.f, false);
    for native in [false, true] {
        assert_eq!(request(native, SELECTED, &valid).status, 403);
        unchanged(&initial);
    }
    c.catalog.check_grants(&c.f, false);
    c.catalog.membership(&c.f, true);
    for sql in [
        "UPDATE board_policies SET active=$2 WHERE board_id=$1",
        "UPDATE workspaces SET active=$2 WHERE id=(SELECT workspace_id FROM board_policies WHERE board_id=$1)",
    ] {
        c.f.connect().execute(sql, &[&SELECTED, &false]).unwrap();
        for native in [false, true] {
            assert_eq!(request(native, SELECTED, &valid).status, 403);
            unchanged(&initial);
        }
        c.f.connect().execute(sql, &[&SELECTED, &true]).unwrap();
    }
    assert_eq!(
        c.request(None, "POST", &route(SELECTED, false), &valid)
            .status,
        401
    );
    for path in [
        format!("/navigation/boards/{SELECTED}/items/archive-selection"),
        format!("/boards/{SELECTED}/main-table/items/archive-selection"),
    ] {
        assert_eq!(
            send(
                native.port,
                "POST",
                &path,
                &format!("127.0.0.1:{}", native.port),
                &[],
                &valid.to_string()
            )
            .status,
            404
        );
    }
    unchanged(&initial);
    c.f.connect().batch_execute("CREATE FUNCTION refuse_selected_archive() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER refuse_selected_archive AFTER INSERT ON group_reorder_receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION refuse_selected_archive()").unwrap();
    for native in [false, true] {
        assert_eq!(request(native, SELECTED, &valid).status, 500);
        unchanged(&initial);
    }
    c.f.connect()
        .batch_execute("DROP TRIGGER refuse_selected_archive ON group_reorder_receipts")
        .unwrap();
    // Independent native/preview sessions contend at the same board baseline.
    let rival_op = Uuid::new_v4();
    let rival = draft(rival_op, 1, &[(67, 1), (59, 1), (61, 1)]);
    let start = std::sync::Barrier::new(2);
    let (left, right) = std::thread::scope(|scope| {
        let fork = scope.spawn(|| {
            start.wait();
            request(true, SELECTED, &rival)
        });
        start.wait();
        (request(false, SELECTED, &valid), fork.join().unwrap())
    });
    let (ack, loser, valid, op) = if left.status == 200 {
        (left, right, valid, op)
    } else {
        (right, left, rival, rival_op)
    };
    assert_eq!(ack.status, 200);
    assert_eq!(loser.status, 409);
    let saved = initial.archived_selection(&c.f, c.member, op, &[67, 59, 61]);
    for native in [true, false] {
        let replay = request(native, SELECTED, &valid);
        assert_eq!(replay.status, 200);
        assert_eq!(replay.body, ack.body);
        unchanged(&saved);
        for body in [
            draft(op, 1, &[(59, 1), (67, 1), (61, 1)]),
            draft(op, 2, &[(67, 1), (59, 1), (61, 1)]),
            draft(op, 1, &[(67, 2), (59, 1), (61, 1)]),
            draft(op, 1, &[(67, 1)]),
        ] {
            assert_eq!(request(native, SELECTED, &body).status, 409);
            unchanged(&saved);
        }
    }
    // Single-root identities must not acknowledge a batch or vice versa.
    let mut db = c.f.connect();
    let mut tx = db.transaction().unwrap();
    assert!(matches!(
        store::transition_root(
            &mut tx,
            c.member,
            BoardId::new(SELECTED).unwrap(),
            ItemId::new(Uuid::from_u128(67)).unwrap(),
            &store::RootCommand {
                action: store::Lifecycle::Archive,
                operation: op,
                board_revision: 1,
                item_revision: 1
            }
        ),
        Err(store::ReorderError::Conflict)
    ));
    tx.rollback().unwrap();
    unchanged(&saved);
    let client =
        boardcraft_mobile_edit_client::EditClient::with_bearer(native.port, token.encoded())
            .unwrap();
    assert_eq!(
        client
            .read_main_table(SELECTED)
            .unwrap()
            .groups()
            .iter()
            .flat_map(|g| g.items())
            .map(|r| r.id.as_u128())
            .collect::<Vec<_>>(),
        [73, 77]
    );
    let page = client.read_main_table_archive(SELECTED, None).unwrap();
    assert_eq!(page.total(), 4);
    assert_eq!(
        page.items()
            .iter()
            .map(|r| r.id().as_u128())
            .collect::<Vec<_>>(),
        [57, 59, 61, 67]
    );
    for name in ["member", "viewer"] {
        role(name);
        for native in [false, true] {
            assert_eq!(request(native, SELECTED, &valid).status, 403);
            unchanged(&saved);
        }
    }
    role("owner");
    c.catalog.membership(&c.f, false);
    for native in [false, true] {
        assert_eq!(request(native, SELECTED, &valid).status, 403);
        unchanged(&saved);
    }
    c.catalog.membership(&c.f, true);
    // A different authorized Owner cannot claim the actor-bound receipt.
    let password = Uuid::new_v4().to_string();
    let peer =
        c.f.seed_account("archive-peer", &password, boardcraft_account::Role::Owner);
    c.f.connect().execute("INSERT INTO workspace_members SELECT workspace_id,$2 FROM board_policies WHERE board_id=$1",&[&SELECTED,&peer]).unwrap();
    c.f.connect()
        .execute(
            "INSERT INTO board_subscribers VALUES($1,$2)",
            &[&SELECTED, &peer],
        )
        .unwrap();
    let pair = crate::create::login(c.host.port, "archive-peer", &password);
    assert_eq!(
        c.request(Some(&pair), "POST", &route(SELECTED, false), &valid)
            .status,
        409
    );
    unchanged(&saved);
    assert_eq!(
        c.request(
            Some(&c.pair),
            "POST",
            "/preview/api/session/logout",
            &Value::Null
        )
        .status,
        200
    );
    assert_eq!(request(false, SELECTED, &valid).status, 401);
    unchanged(&saved);
    boardcraft_account::revoke(&c.f.schema.config, &token).unwrap();
    assert_eq!(request(true, SELECTED, &valid).status, 401);
    unchanged(&saved);
    migration();
}

fn migration() {
    use crate::fixture::Schema;
    let schema = Schema::new();
    let mut db = schema.config.connect(postgres::NoTls).unwrap();
    for sql in [
        include_str!("../../../board/migrations/0001_boards.sql"),
        include_str!("../../../board/migrations/0002_item_revision.sql"),
        include_str!("../../../board/migrations/0003_board_groups.sql"),
        include_str!("../../../board/migrations/0004_item_creation.sql"),
        include_str!("../../../board/migrations/0005_item_group_move.sql"),
        include_str!("../../../board/migrations/0006_group_reorder.sql"),
        include_str!("../../../board/migrations/0007_selection_move.sql"),
        include_str!("../../../board/migrations/0008_root_lifecycle.sql"),
        include_str!("../../../board/migrations/0009_group_lifecycle.sql"),
        include_str!("../../../board/migrations/0010_group_colour.sql"),
        include_str!("../../../board/migrations/0011_main_table.sql"),
    ] {
        db.batch_execute(sql).unwrap();
    }
    db.batch_execute("CREATE TABLE rustday_migrations(version integer PRIMARY KEY); INSERT INTO rustday_migrations SELECT generate_series(1,11)").unwrap();
    db.execute("INSERT INTO boards(id) VALUES($1)", &[&SELECTED])
        .unwrap();
    db.execute(
        "INSERT INTO board_groups(board_id,id,name,position,active) VALUES($1,'g','Work',0,true)",
        &[&SELECTED],
    )
    .unwrap();
    let actor = Uuid::new_v4();
    let single = Uuid::new_v4();
    let moving = Uuid::new_v4();
    let item = Uuid::from_u128(67);
    let other = Uuid::from_u128(59);
    for id in [item, other] {
        db.execute("INSERT INTO board_items(board_id,id,group_id,title,position) VALUES($1,$2,'g','Ink',30)",&[&SELECTED,&id]).unwrap();
    }
    let board = BoardId::new(SELECTED).unwrap();
    let command = store::RootCommand {
        action: store::Lifecycle::Archive,
        operation: single,
        board_revision: 1,
        item_revision: 1,
    };
    let mut tx = db.transaction().unwrap();
    store::transition_root(&mut tx, actor, board, ItemId::new(item).unwrap(), &command).unwrap();
    tx.commit().unwrap();
    db.execute("INSERT INTO group_reorder_receipts(operation_id,actor_id,board_id,group_id,board_revision,selected_ids,selected_revisions,affected_ids) VALUES($1,$2,$3,'g',1,ARRAY[$4::uuid],ARRAY[1],ARRAY[$4::uuid])",&[&moving,&actor,&SELECTED,&other]).unwrap();
    let rows = |db: &mut postgres::Client| {
        db.query(
            "SELECT row_to_json(i)::text FROM board_items i ORDER BY id",
            &[],
        )
        .unwrap()
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect::<Vec<_>>()
    };
    let before = rows(&mut db);
    for _ in 0..2 {
        store::migrate(&mut db).unwrap();
        db = schema.config.connect(postgres::NoTls).unwrap();
        assert_eq!(rows(&mut db), before);
        let mut tx = db.transaction().unwrap();
        store::transition_root(&mut tx, actor, board, ItemId::new(item).unwrap(), &command)
            .unwrap();
        assert_eq!(
            store::move_selected_items(
                &mut tx,
                actor,
                board,
                &store::SelectionMove {
                    operation: moving,
                    group: "g",
                    board_revision: 1,
                    items: &[(other, 1)]
                }
            )
            .unwrap(),
            [other]
        );
        for op in [single, moving] {
            assert!(matches!(
                store::archive_selected_roots(&mut tx, actor, board, op, 2, &[(other, 1)]),
                Err(store::ReorderError::Conflict)
            ));
        }
        tx.commit().unwrap();
        assert_eq!(rows(&mut db), before);
    }
}
