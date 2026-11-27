use super::{
    Window, http_window_command, main_table_relay::Relay, native_archive,
    native_selection::NativeSelection, selection::SELECTED,
};
use std::sync::atomic::Ordering::SeqCst;
use uuid::Uuid;

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
fn calls(r: &Relay, expected: &[(&str, u16, i64)]) {
    let entries = r.entries.lock().unwrap();
    assert_eq!(entries.len(), expected.len());
    assert_eq!(r.saves.load(SeqCst), expected.len());
    let mut operations = Vec::new();
    for (call, &(action, status, revision)) in entries.iter().zip(expected) {
        let root = action.starts_with("item-");
        let verb = action.strip_prefix("item-").unwrap_or(action);
        assert_eq!(call.method, "POST");
        assert_eq!(
            call.path,
            format!(
                "/navigation/boards/{SELECTED}/main-table/{}/{verb}",
                if root {
                    format!("items/{}", Uuid::from_u128(67))
                } else {
                    "groups/%67%2D%39%30".to_owned()
                }
            )
        );
        assert_eq!(call.status, status);
        assert_eq!(call.body["board_revision"], revision);
        if root {
            assert_eq!(
                call.body["item_revision"],
                if verb == "archive" { 1 } else { 2 }
            );
            let operation: Uuid =
                serde_json::from_value(call.body["operation_id"].clone()).unwrap();
            assert!(!operation.is_nil());
            assert!(!operations.contains(&operation));
            operations.push(operation);
        } else {
            assert_eq!(call.body, serde_json::json!({"board_revision":revision}));
        }
    }
}
pub(super) fn round_trip() {
    let f = native_archive::fixture();
    let mut state = f.state();
    let r = Relay::start(f.selection.host.port);
    let mut w = launch(&f, &r, "--archive-round-trip");
    let mut expected = Vec::new();
    let mut lock = f.f.connect();
    for point in [
        "opened",
        "cancel-ready",
        "cancelled",
        "confirming",
        "pending",
        "root-archived",
        "group-cancelled",
        "group-confirming",
        "group-pending",
        "group-archived",
        "dismissed",
        "revisited",
        "inventory",
        "group-restored",
        "item-restored",
        "active-restored",
        "undo-ready",
        "undone",
        "reloaded",
    ] {
        w.expect(point);
        state = match point {
            "root-archived" => {
                expected.push(("item-archive", 200, 1));
                state.lifecycle_records(&f.f, f.member, "archive")
            }
            "group-archived" => {
                expected.push(("archive", 200, 2));
                state.group_transition(&f.f, "g-90", false)
            }
            "group-restored" => {
                expected.push(("restore", 200, 3));
                state.group_transition(&f.f, "g-90", true)
            }
            "item-restored" => {
                expected.push(("item-restore", 200, 4));
                state.lifecycle_records(&f.f, f.member, "restore")
            }
            "undo-ready" => {
                expected.push(("archive", 200, 5));
                state.group_transition(&f.f, "g-90", false)
            }
            "undone" => {
                expected.push(("restore", 200, 6));
                state.group_transition(&f.f, "g-90", true)
            }
            _ => {
                assert_eq!(f.state(), state);
                state
            }
        };
        f.check_access("owner", true, true);
        let commands: Vec<Uuid> = r
            .entries
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| e.body.get("operation_id"))
            .map(|id| serde_json::from_value(id.clone()).unwrap())
            .collect();
        let receipts: Vec<Uuid> = f.f.connect().query("SELECT operation_id FROM group_reorder_receipts WHERE lifecycle IS NOT NULL ORDER BY board_revision", &[]).unwrap().iter().map(|r| r.get(0)).collect();
        assert_eq!(receipts, commands);
        if matches!(point, "confirming" | "group-confirming") {
            lock.batch_execute("BEGIN").unwrap();
            lock.query_one("SELECT id FROM boards WHERE id=$1 FOR UPDATE", &[&SELECTED])
                .unwrap();
        }
        if matches!(point, "pending" | "group-pending") {
            super::grouped_entry::wait_for_write(&mut lock);
            assert_eq!(r.saves.load(SeqCst), expected.len() + 1);
            lock.batch_execute("COMMIT").unwrap();
        } else {
            calls(&r, &expected);
        }
        if point == "inventory" {
            assert_eq!(r.archive_reads.load(SeqCst), 3);
        }
        if point == "reloaded" {
            assert_eq!(r.archive_reads.load(SeqCst), 7);
            w.finish();
            return;
        }
        w.reply("ok");
    }
}
pub(super) fn recovery() {
    for mode in [
        "--archive-conflict",
        "--archive-demoted",
        "--archive-denied",
        "--archive-session",
        "--archive-ack",
        "--archive-refresh",
        "--archive-member",
        "--archive-viewer",
    ] {
        let f = native_archive::fixture();
        let client = f.client();
        client
            .archive_main_table_item(SELECTED, Uuid::from_u128(67), Uuid::new_v4(), 1, 1, false)
            .unwrap();
        client
            .archive_main_table_group(SELECTED, "g-90", 2, false)
            .unwrap();
        let mut state = f.state();
        if mode == "--archive-member" {
            f.role("member");
        }
        if mode == "--archive-viewer" {
            f.role("viewer");
        }
        let r = Relay::start(f.selection.host.port);
        let mut w = launch(&f, &r, mode);
        w.expect("opened");
        assert_eq!(f.state(), state);
        w.reply("ok");
        w.expect("inventory");
        assert_eq!(f.state(), state);
        calls(&r, &[]);
        assert_eq!(r.reads.load(SeqCst), 2);
        assert_eq!(r.archive_reads.load(SeqCst), 1);
        match mode {
            "--archive-conflict" => {
                f.f.connect()
                    .execute(
                        "UPDATE boards SET revision=revision+1 WHERE id=$1",
                        &[&SELECTED],
                    )
                    .unwrap();
                state = f.state();
            }
            "--archive-demoted" => f.role("member"),
            "--archive-denied" => f.selection.membership(&f.f, false),
            "--archive-session" => f.selection.session.revoke(&f.f),
            "--archive-ack" => r.fault.store(2, SeqCst),
            "--archive-refresh" => r.fault.store(1, SeqCst),
            _ => {
                f.check_access(
                    if mode == "--archive-member" {
                        "member"
                    } else {
                        "viewer"
                    },
                    true,
                    true,
                );
                w.finish();
                continue;
            }
        }
        w.reply("ok");
        match mode {
            "--archive-conflict" => {
                for point in ["refused", "reviewed"] {
                    w.expect(point);
                    assert_eq!(f.state(), state);
                    calls(&r, &[("restore", 409, 3)]);
                    if point == "reviewed" {
                        assert_eq!(r.reads.load(SeqCst), 3);
                        assert_eq!(r.archive_reads.load(SeqCst), 2);
                    }
                    w.reply("ok");
                }
                w.expect("restored");
                state.group_transition(&f.f, "g-90", true);
                calls(&r, &[("restore", 409, 3), ("restore", 200, 4)]);
                w.finish();
            }
            "--archive-demoted" | "--archive-denied" | "--archive-session" => {
                w.expect(if mode == "--archive-demoted" {
                    "denied"
                } else if mode == "--archive-denied" {
                    "unavailable"
                } else {
                    "expired"
                });
                assert_eq!(f.state(), state);
                calls(
                    &r,
                    &[(
                        "restore",
                        if mode == "--archive-session" {
                            401
                        } else {
                            403
                        },
                        3,
                    )],
                );
                f.check_access(
                    if mode == "--archive-demoted" {
                        "member"
                    } else {
                        "owner"
                    },
                    mode != "--archive-denied",
                    mode != "--archive-session",
                );
                assert_eq!(
                    r.reads.load(SeqCst),
                    if mode == "--archive-session" { 2 } else { 3 }
                );
                assert_eq!(
                    r.archive_reads.load(SeqCst),
                    if mode == "--archive-demoted" { 2 } else { 1 }
                );
                w.finish();
            }
            _ => {
                w.expect("uncertain");
                let saved = state.group_transition(&f.f, "g-90", true);
                calls(&r, &[("restore", 200, 3)]);
                let reads = r.reads.load(SeqCst);
                w.reply("ok");
                w.expect("reconciled");
                assert_eq!(f.state(), saved);
                calls(&r, &[("restore", 200, 3)]);
                assert_eq!(r.reads.load(SeqCst), reads + 1);
                assert_eq!(r.archive_reads.load(SeqCst), 2);
                w.finish();
            }
        }
    }
}
