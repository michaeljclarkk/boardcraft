use crate::{
    browser_transport, native_placement,
    selection::{HIDDEN, SELECTED},
};
use boardcraft_mobile_edit_client::{EditClient, EditError, ItemReorder, NameEdit};
use serde_json::json;
use uuid::Uuid;

pub fn access() {
    for reorder in [false, true] {
        let f = native_placement::fixture(reorder);
        let c = f.client();
        let item = Uuid::from_u128(if reorder { 67 } else { 61 });
        let body = if reorder {
            json!({"operation_id":Uuid::new_v4(),"group_id":"g-90","anchor_id":Uuid::from_u128(61),"side":"before","board_revision":1,"item_revision":1})
        } else {
            json!({"group_id":"g-20","expected_revision":1})
        };
        let method = if reorder { "POST" } else { "PUT" };
        let field = if reorder { "reorder" } else { "group" };
        let path = format!("/navigation/boards/{SELECTED}/main-table/items/{item}/{field}");
        let send = |port, method, path: &str, authorized, body: &serde_json::Value| {
            let bearer = format!("Bearer {}", f.selection.token.encoded());
            let fields = [("Authorization", bearer.as_str())];
            browser_transport::send(
                port,
                method,
                path,
                &format!("127.0.0.1:{port}"),
                if authorized { &fields } else { &[] },
                &body.to_string(),
            )
            .status
        };
        let post = |path: &str, body: &serde_json::Value| {
            send(f.selection.host.port, method, path, true, body)
        };
        let unchanged = |state: &crate::reorder_state::Snapshot| assert_eq!(f.state(), *state);
        let mut state = f.state();
        assert_eq!(
            send(f.selection.host.port, method, &path, false, &body),
            401
        );
        assert_eq!(send(f.f.host.port, method, &path, true, &body), 404);
        for (m, p) in [
            ("GET", path.clone()),
            (if reorder { "PUT" } else { "POST" }, path.clone()),
            (method, path.replace("/main-table", "")),
            (method, path.replace("/navigation", "")),
        ] {
            assert_eq!(send(f.selection.host.port, m, &p, true, &body), 404);
        }
        assert_eq!(
            post(
                &path.replace(&SELECTED.to_string(), &HIDDEN.to_string()),
                &body
            ),
            403
        );
        f.role("viewer");
        assert_eq!(post(&path, &body), 403);
        unchanged(&state);
        f.role("member");
        f.selection.membership(&f.f, false);
        assert_eq!(post(&path, &body), 403);
        unchanged(&state);
        f.selection.membership(&f.f, true);
        f.f.connect()
            .execute(
                "DELETE FROM board_subscribers WHERE board_id=$1 AND account_id=$2",
                &[&SELECTED, &f.member],
            )
            .unwrap();
        assert_eq!(post(&path, &body), 403);
        unchanged(&state);
        f.f.connect()
            .execute(
                "INSERT INTO board_subscribers VALUES ($1,$2)",
                &[&SELECTED, &f.member],
            )
            .unwrap();
        for sql in [
            "UPDATE board_policies SET active=$2 WHERE board_id=$1",
            "UPDATE workspaces SET active=$2 WHERE id=(SELECT workspace_id FROM board_policies WHERE board_id=$1)",
        ] {
            f.f.connect().execute(sql, &[&SELECTED, &false]).unwrap();
            assert_eq!(post(&path, &body), 403);
            unchanged(&state);
            f.f.connect().execute(sql, &[&SELECTED, &true]).unwrap();
        }
        for n in [57, 62, 17, 88] {
            assert_eq!(
                post(
                    &path.replace(&item.to_string(), &Uuid::from_u128(n).to_string()),
                    &body
                ),
                404
            );
            unchanged(&state);
        }
        for group in ["missing", "g-30", "g-70"] {
            let mut bad = body.clone();
            bad["group_id"] = json!(group);
            assert_eq!(post(&path, &bad), 422);
            unchanged(&state);
        }
        for key in if reorder {
            vec!["board_revision", "item_revision"]
        } else {
            vec!["expected_revision"]
        } {
            for value in [0, 99] {
                let mut bad = body.clone();
                bad[key] = json!(value);
                assert_eq!(post(&path, &bad), 409);
                unchanged(&state);
            }
        }
        let mut bad = body.clone();
        bad["position"] = json!(0);
        assert_eq!(post(&path, &bad), 400);
        unchanged(&state);
        if reorder {
            for (n, status) in [(67, 422), (59, 409), (62, 409), (88, 409)] {
                let mut bad = body.clone();
                bad["anchor_id"] = json!(Uuid::from_u128(n));
                assert_eq!(post(&path, &bad), status);
                unchanged(&state);
            }
            let mut bad = body.clone();
            bad["side"] = json!("first");
            assert_eq!(post(&path, &bad), 400);
            unchanged(&state);
        }
        c.read_main_table(SELECTED).unwrap();
        let group = if reorder { "g-90" } else { "g-20" };
        f.f.connect()
            .execute(
                "UPDATE board_groups SET active=false WHERE board_id=$1 AND id=$2",
                &[&SELECTED, &group],
            )
            .unwrap();
        state = f.state();
        assert_eq!(post(&path, &body), 422);
        unchanged(&state);
        f.f.connect()
            .execute(
                "UPDATE board_groups SET active=true WHERE board_id=$1 AND id=$2",
                &[&SELECTED, &group],
            )
            .unwrap();
        state = f.state();
        f.f.connect()
            .batch_execute(if reorder {
                native_placement::ORDER_FAULT
            } else {
                native_placement::MOVE_FAULT
            })
            .unwrap();
        assert_eq!(post(&path, &body), 500);
        unchanged(&state);
        f.f.connect()
            .batch_execute(if reorder {
                "DROP TRIGGER refuse_placement ON group_reorder_receipts"
            } else {
                "DROP TRIGGER refuse_placement ON board_items"
            })
            .unwrap();
        f.role("owner");
        assert_eq!(post(&path, &body), 200);
        if reorder {
            state.check(&f.f, f.member, 1);
            let committed = state.reordered_root(&f.f, f.member, false);
            assert_eq!(post(&path, &body), 200); // Exact receipt replay, still authorized.
            unchanged(&committed);
            let mut reused = body.clone();
            reused["side"] = json!("after");
            assert_eq!(post(&path, &reused), 409);
            assert_eq!(
                c.move_main_table_item(SELECTED, item, "g-20", 1),
                Err(EditError::Conflict)
            );
            f.role("member");
            c.reorder_main_table_item(
                SELECTED,
                item,
                &ItemReorder::new(Uuid::new_v4(), "g-90", Uuid::from_u128(61), true, 2, 2),
            )
            .unwrap();
            state.check(&f.f, f.member, 2);
            let committed = committed.reordered_root(&f.f, f.member, true);
            f.role("viewer");
            assert_eq!(post(&path, &body), 403); // Replay does not retain old write access.
            unchanged(&committed);
            f.role("member");
            c.move_main_table_item(SELECTED, Uuid::from_u128(61), "g-20", 1)
                .unwrap();
            let moved = committed.moved_root(&f.f, 61, "g-20");
            assert_eq!(
                c.reorder_main_table_item(
                    SELECTED,
                    item,
                    &ItemReorder::new(Uuid::new_v4(), "g-90", Uuid::from_u128(61), false, 3, 3)
                ),
                Err(EditError::Conflict)
            );
            unchanged(&moved);
        } else {
            state = state.moved_root(&f.f, 61, "g-20");
            // Current same-value save is a no-op. A name edit may cross a group baseline.
            c.move_main_table_item(SELECTED, item, "g-20", 2).unwrap();
            unchanged(&state);
            c.rename_main_table_item(
                SELECTED,
                item,
                &NameEdit {
                    title: "Coral".into(),
                    revision: 2,
                },
            )
            .unwrap();
            state = f.state();
            f.role("member");
            c.move_main_table_item(SELECTED, item, "g-90", 2).unwrap();
            state = state.moved_root(&f.f, 61, "g-90");
            assert_eq!(
                c.move_main_table_item(SELECTED, item, "g-90", 2),
                Err(EditError::Conflict)
            );
            let read = EditClient::with_bearer(f.selection.host.port, f.selection.token.encoded())
                .unwrap()
                .read_main_table(SELECTED)
                .unwrap();
            assert_eq!(
                read.groups()[0]
                    .items()
                    .iter()
                    .map(|r| r.id.as_u128())
                    .collect::<Vec<_>>(),
                [73, 61]
            );
            assert_eq!(read.groups()[0].items()[1].title, "Coral");
            unchanged(&state);
        }
        let state = f.state();
        f.selection.session.revoke(&f.f);
        assert_eq!(post(&path, &body), 401);
        unchanged(&state);
    }
}
