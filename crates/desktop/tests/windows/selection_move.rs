use super::{
    Window, http_window_command, main_table_relay::Relay, native_selection::NativeSelection,
    selection::SELECTED,
};
use boardcraft_mobile_edit_client::NameEdit;
use std::sync::atomic::Ordering::SeqCst;
use uuid::Uuid;

fn fixture() -> NativeSelection {
    let f = NativeSelection::new();
    f.client()
        .set_main_table_fold(SELECTED, 1, &["g-10".to_owned()], true)
        .unwrap();
    f
}
fn launch(f: &NativeSelection, r: &Relay, mode: &str) -> Window {
    let mut cmd = http_window_command();
    cmd.args(["browse", &r.port.to_string(), mode])
        .env(
            "BOARDCRAFT_SESSION_ORIGIN",
            format!("http://127.0.0.1:{}", r.port),
        )
        .env("BOARDCRAFT_SESSION_TOKEN", f.selection.token.encoded());
    Window::launch(cmd)
}
fn calls(r: &Relay, statuses: &[u16], revision: i64, reviewed: bool) {
    let entries = r.entries.lock().unwrap();
    assert_eq!(entries.len(), statuses.len());
    let mut op = None;
    for (ix, (e, status)) in entries.iter().zip(statuses).enumerate() {
        assert_eq!(e.method, "POST");
        assert_eq!(
            e.path,
            format!("/navigation/boards/{SELECTED}/main-table/items/move-selection")
        );
        assert_eq!(e.status, *status);
        assert_eq!(e.body["group_id"], "g-90");
        assert_eq!(e.body["board_revision"], revision);
        let command: Vec<_> = e.body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| (i["id"].clone(), i["revision"].as_i64().unwrap()))
            .collect();
        assert_eq!(
            command,
            [67, 59, 61].map(|n| (
                serde_json::json!(Uuid::from_u128(n)),
                if reviewed && ix > 0 && n == 61 { 2 } else { 1 }
            ))
        );
        let id: Uuid = serde_json::from_value(e.body["operation_id"].clone()).unwrap();
        assert!(!id.is_nil());
        assert!(op.is_none_or(|old| old == id));
        op = Some(id);
    }
}
pub(super) fn moves() {
    let f = fixture();
    let mut state = f.state();
    let r = Relay::start(f.selection.host.port);
    let mut w = launch(&f, &r, "--selection-move");
    let mut lock = f.f.connect();
    for point in [
        "opened",
        "hidden",
        "hidden-range",
        "heading-selected",
        "expanded",
        "cancel-draft",
        "cancelled",
        "confirming",
        "pending",
        "saved",
        "group-order",
        "reloaded",
    ] {
        w.expect(point);
        state = match point {
            "hidden" => state.folded(&f.f, 3, &["g-10", "g-20"]),
            "expanded" => state.folded(&f.f, 4, &["g-10"]),
            "saved" => state.batch(&f.f, f.member, "g-90", &[73, 77, 67, 59, 61], &[67, 59, 61]),
            _ => {
                assert_eq!(f.state(), state);
                state
            }
        };
        assert!(f.selection.session.present(&f.f));
        f.check_access("member", true, true);
        assert_eq!(
            r.reads.load(SeqCst),
            match point {
                "opened" => 1,
                "hidden" => 2,
                "hidden-range" | "heading-selected" => 3,
                "saved" => 5,
                "group-order" => 6,
                "reloaded" => 7,
                _ => 4,
            }
        );
        calls(
            &r,
            if matches!(point, "saved" | "group-order" | "reloaded") {
                &[200]
            } else {
                &[]
            },
            4,
            false,
        );
        if point == "confirming" {
            lock.batch_execute("BEGIN").unwrap();
            lock.query_one("SELECT id FROM boards WHERE id=$1 FOR UPDATE", &[&SELECTED])
                .unwrap();
        }
        if point == "pending" {
            super::grouped_entry::wait_for_write(&mut lock);
            assert_eq!(r.saves.load(SeqCst), 3);
            lock.batch_execute("COMMIT").unwrap();
        }
        if point == "reloaded" {
            w.finish();
            return;
        }
        w.reply("ok");
    }
}
pub(super) fn reviews() {
    for mode in [
        "--selection-conflict",
        "--selection-missing",
        "--selection-demoted",
        "--selection-denied",
        "--selection-ack",
        "--selection-refresh",
        "--selection-viewer",
        "--selection-viewer-range",
    ] {
        let f = fixture();
        let mut state = f.state();
        let r = Relay::start(f.selection.host.port);
        if mode.starts_with("--selection-viewer") {
            f.role("viewer");
        }
        let mut w = launch(&f, &r, mode);
        w.expect("opened");
        f.check_access(
            if mode.starts_with("--selection-viewer") {
                "viewer"
            } else {
                "member"
            },
            true,
            true,
        );
        assert_eq!(f.state(), state);
        w.reply("ok");
        if mode == "--selection-viewer-range" {
            for point in [
                "range-draft",
                "range-refused",
                "range-retried",
                "range-cleared",
            ] {
                w.expect(point);
                assert_eq!(f.state(), state);
                assert_eq!(r.saves.load(SeqCst), 0);
                assert_eq!(
                    r.reads.load(SeqCst),
                    match point {
                        "range-draft" => 1,
                        "range-refused" => 2,
                        _ => 3,
                    }
                );
                if point == "range-draft" {
                    r.fault.store(1, SeqCst);
                }
                if point == "range-cleared" {
                    w.finish();
                    break;
                }
                w.reply("ok");
            }
            continue;
        }
        if mode == "--selection-viewer" {
            w.expect("viewer");
            assert_eq!(f.state(), state);
            assert_eq!(r.saves.load(SeqCst), 0);
            w.reply("ok");
            w.expect("cleared");
            assert_eq!(f.state(), state);
            w.finish();
            continue;
        }
        w.expect("draft");
        assert_eq!(f.state(), state);
        assert_eq!(r.saves.load(SeqCst), 0);
        match mode {
            "--selection-conflict" => {
                f.client()
                    .rename_main_table_item(
                        SELECTED,
                        Uuid::from_u128(61),
                        &NameEdit {
                            title: "Indigo".into(),
                            revision: 1,
                        },
                    )
                    .unwrap();
                state = f.state();
            }
            "--selection-missing" => {
                f.f.connect()
                    .execute(
                        "UPDATE board_items SET state=1,revision=revision+1 WHERE id=$1",
                        &[&Uuid::from_u128(61)],
                    )
                    .unwrap();
                state = f.state();
            }
            "--selection-demoted" => f.role("viewer"),
            "--selection-denied" => f.selection.membership(&f.f, false),
            "--selection-ack" => r.fault.store(2, SeqCst),
            "--selection-refresh" => r.fault.store(1, SeqCst),
            _ => unreachable!(),
        }
        w.reply("ok");
        match mode {
            "--selection-missing" => {
                w.expect("conflict");
                assert_eq!(f.state(), state);
                calls(&r, &[404], 2, false);
                w.reply("ok");
                w.expect("missing-reviewed");
                assert_eq!(f.state(), state);
                calls(&r, &[404], 2, false);
                w.finish();
            }
            "--selection-conflict" => {
                for point in ["conflict", "reviewed", "adopted"] {
                    w.expect(point);
                    assert_eq!(f.state(), state);
                    calls(&r, &[409], 2, true);
                    w.reply("ok");
                }
                w.expect("deliberate");
                state.batch(&f.f, f.member, "g-90", &[73, 77, 67, 59, 61], &[67, 59, 61]);
                calls(&r, &[409, 200], 2, true);
                assert_eq!(r.reads.load(SeqCst), 3);
                w.finish();
            }
            "--selection-demoted" | "--selection-denied" => {
                w.expect(if mode == "--selection-demoted" {
                    "denied"
                } else {
                    "unavailable"
                });
                assert_eq!(f.state(), state);
                f.check_access(
                    if mode == "--selection-demoted" {
                        "viewer"
                    } else {
                        "member"
                    },
                    mode != "--selection-denied",
                    true,
                );
                calls(&r, &[403], 2, false);
                assert_eq!(r.reads.load(SeqCst), 2);
                if mode == "--selection-demoted" {
                    w.reply("ok");
                    w.expect("cleared");
                    assert_eq!(f.state(), state);
                    f.role("member");
                    w.reply("ok");
                    w.expect("draft-restored");
                    assert_eq!(f.state(), state);
                    calls(&r, &[403], 2, false);
                    assert_eq!(r.reads.load(SeqCst), 3);
                }
                w.finish();
            }
            _ => {
                w.expect("uncertain");
                let saved =
                    state.batch(&f.f, f.member, "g-90", &[73, 77, 67, 59, 61], &[67, 59, 61]);
                calls(&r, &[200], 2, false);
                let reads = r.reads.load(SeqCst);
                w.reply("ok");
                w.expect("reconciled");
                assert_eq!(f.state(), saved);
                calls(&r, &[200], 2, false);
                assert_eq!(r.reads.load(SeqCst), reads + 1);
                w.finish();
            }
        }
    }
}
