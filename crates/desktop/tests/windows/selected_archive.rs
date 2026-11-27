use super::{
    Window, http_window_command, main_table_relay::Relay, native_archive, selection::SELECTED,
};
use std::sync::atomic::Ordering::SeqCst;
use uuid::Uuid;

pub(super) fn run() {
    for mode in [
        "success",
        "conflict",
        "commit",
        "missing",
        "review-failed",
        "demoted",
        "denied",
        "session",
        "ack",
        "refresh",
        "member",
        "viewer",
    ] {
        let f = native_archive::fixture();
        if matches!(mode, "member" | "viewer") {
            f.role(mode);
        }
        let r = Relay::start(f.selection.host.port);
        let mut cmd = http_window_command();
        cmd.args([
            "browse",
            &r.port.to_string(),
            &format!("--bulk-archive-{mode}"),
        ])
        .env(
            "BOARDCRAFT_SESSION_ORIGIN",
            format!("http://127.0.0.1:{}", r.port),
        )
        .env("BOARDCRAFT_SESSION_TOKEN", f.selection.token.encoded());
        let mut w = Window::launch(cmd);
        let mut state = f.state();
        w.expect("opened");
        assert_eq!(f.state(), state);
        w.reply("ok");
        if matches!(mode, "member" | "viewer") {
            w.expect("readonly");
            assert_eq!(f.state(), state);
            assert_eq!(r.saves.load(SeqCst), 0);
            w.finish();
            continue;
        }
        w.expect("folded");
        state = state.folded(&f.f, 2, &["g-20"]);
        w.reply("ok");
        if mode == "success" {
            for point in ["cancel-ready", "cancelled"] {
                w.expect(point);
                assert_eq!(f.state(), state);
                assert!(r.entries.lock().unwrap().is_empty());
                w.reply("ok");
            }
        }
        w.expect("draft");
        assert_eq!(f.state(), state);
        let mut lock = f.f.connect();
        match mode {
            "success" => {
                lock.batch_execute("BEGIN").unwrap();
                lock.query_one("SELECT id FROM boards WHERE id=$1 FOR UPDATE", &[&SELECTED])
                    .unwrap();
            }
            "conflict" | "review-failed" => {
                f.client()
                    .rename_main_table_item(
                        SELECTED,
                        Uuid::from_u128(61),
                        &boardcraft_mobile_edit_client::NameEdit {
                            title: "Indigo".into(),
                            revision: 1,
                        },
                    )
                    .unwrap();
                state = f.state();
            }
            "missing" => {
                f.f.connect()
                    .execute(
                        "UPDATE board_items SET state=1,revision=revision+1 WHERE id=$1",
                        &[&Uuid::from_u128(61)],
                    )
                    .unwrap();
                state = f.state();
            }
            "commit" => {
                f.f.connect().batch_execute("CREATE FUNCTION refuse_bulk() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER refuse_bulk AFTER INSERT ON group_reorder_receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION refuse_bulk()").unwrap();
            }
            "demoted" => f.role("member"),
            "denied" => f.selection.membership(&f.f, false),
            "session" => f.selection.session.revoke(&f.f),
            "ack" => r.fault.store(2, SeqCst),
            "refresh" => r.fault.store(1, SeqCst),
            _ => unreachable!(),
        }
        w.reply("ok");
        if mode == "success" {
            w.expect("pending");
            assert_eq!(f.state(), state);
            super::grouped_entry::wait_for_write(&mut lock);
            lock.batch_execute("COMMIT").unwrap();
            w.reply("ok");
            w.expect("saved");
            let op = operation(&r, 0, 200);
            state = state.archived_selection(&f.f, f.member, op, &[67, 59, 61]);
            w.reply("ok");
            w.expect("inventory");
            assert_eq!(f.state(), state);
            w.reply("ok");
            w.expect("restored");
            state = state.lifecycle_records(&f.f, f.member, "restore");
            w.reply("ok");
            w.expect("reloaded");
            assert_eq!(f.state(), state);
            assert_eq!(r.entries.lock().unwrap().len(), 2);
            w.finish();
        } else if matches!(mode, "conflict" | "commit" | "missing" | "review-failed") {
            w.expect("refused");
            assert_eq!(f.state(), state);
            let op = operation(&r, 0, if mode == "commit" { 500 } else { 409 });
            if mode == "commit" {
                f.f.connect()
                    .batch_execute("DROP TRIGGER refuse_bulk ON group_reorder_receipts")
                    .unwrap();
            }
            if mode == "review-failed" {
                r.fault.store(1, SeqCst);
                w.reply("ok");
                w.expect("review-failed");
                assert_eq!(f.state(), state);
                assert_eq!(r.entries.lock().unwrap().len(), 1);
            }
            w.reply("ok");
            w.expect(if mode == "missing" {
                "missing-reviewed"
            } else {
                "reviewed"
            });
            assert_eq!(f.state(), state);
            if mode == "missing" {
                w.finish();
                continue;
            }
            w.reply("ok");
            w.expect("adopted");
            assert_eq!(f.state(), state);
            assert_eq!(r.entries.lock().unwrap().len(), 1);
            w.reply("ok");
            w.expect("deliberate");
            assert_eq!(operation(&r, 1, 200), op);
            state.archived_selection(&f.f, f.member, op, &[67, 59, 61]);
            w.finish();
        } else if matches!(mode, "demoted" | "denied" | "session") {
            w.expect(if mode == "demoted" {
                "denied"
            } else if mode == "denied" {
                "unavailable"
            } else {
                "expired"
            });
            assert_eq!(f.state(), state);
            operation(&r, 0, if mode == "session" { 401 } else { 403 });
            w.finish();
        } else {
            w.expect("uncertain");
            let op = operation(&r, 0, 200);
            state = state.archived_selection(&f.f, f.member, op, &[67, 59, 61]);
            w.reply("ok");
            w.expect("reconciled");
            assert_eq!(f.state(), state);
            assert_eq!(r.entries.lock().unwrap().len(), 1);
            w.finish();
        }
    }
}
fn operation(r: &Relay, ix: usize, status: u16) -> Uuid {
    let calls = r.entries.lock().unwrap();
    let e = &calls[ix];
    assert_eq!(e.method, "POST");
    assert_eq!(
        e.path,
        format!("/navigation/boards/{SELECTED}/main-table/items/archive-selection")
    );
    assert_eq!(e.status, status);
    assert_eq!(e.body["board_revision"], 2);
    assert_eq!(
        e.body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].clone())
            .collect::<Vec<_>>(),
        [67, 59, 61].map(|n| serde_json::json!(Uuid::from_u128(n)))
    );
    let op = serde_json::from_value::<Uuid>(e.body["operation_id"].clone()).unwrap();
    assert!(!op.is_nil());
    op
}
