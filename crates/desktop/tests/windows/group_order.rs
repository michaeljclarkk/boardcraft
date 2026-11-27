use super::{
    Window, http_window_command, main_table_relay::Relay, native_group_order,
    native_selection::NativeSelection, selection::SELECTED,
};
use std::sync::atomic::Ordering::SeqCst;
const ABC: [&str; 3] = ["g-old", "g-z", "g-end"];
const BAC: [&str; 3] = ["g-z", "g-old", "g-end"];
const CBA: [&str; 3] = ["g-end", "g-z", "g-old"];
const ACB: [&str; 3] = ["g-old", "g-end", "g-z"];
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
fn calls(r: &Relay, expected: &[(u16, i64, [&str; 3])]) {
    let entries = r.entries.lock().unwrap();
    assert_eq!(entries.len(), expected.len());
    assert_eq!(r.saves.load(SeqCst), expected.len());
    for (e, (status, revision, ids)) in entries.iter().zip(expected) {
        assert_eq!(e.method, "POST");
        assert_eq!(
            e.path,
            format!("/navigation/boards/{SELECTED}/main-table/group-order")
        );
        assert_eq!(e.status, *status);
        assert_eq!(
            e.body,
            serde_json::json!({"board_revision":revision,"group_ids":ids})
        );
    }
}
pub(super) fn moves() {
    let f = native_group_order::fixture();
    let r = Relay::start(f.selection.host.port);
    let mut state = f.state();
    let mut w = launch(&f, &r, "--order-moves");
    let mut expected = Vec::new();
    let mut lock = f.f.connect();
    for p in [
        "opened",
        "ready",
        "pending",
        "menu-saved",
        "cancelled",
        "invalid",
        "drag-before",
        "scrolled",
        "scroll-stopped",
        "drag-after",
        "revisited",
    ] {
        w.expect(p);
        match p {
            "ready" => {
                lock.batch_execute("BEGIN").unwrap();
                lock.query_one("SELECT id FROM boards WHERE id=$1 FOR UPDATE", &[&SELECTED])
                    .unwrap();
            }
            "pending" => {
                super::grouped_entry::wait_for_write(&mut lock);
                assert_eq!(r.saves.load(SeqCst), 1);
                assert_eq!(f.state(), state);
                lock.batch_execute("COMMIT").unwrap();
            }
            "menu-saved" | "drag-before" | "drag-after" => {
                let ids = if p == "drag-before" { CBA } else { BAC };
                expected.push((200, 7 + expected.len() as i64, ids));
                state = state.reordered_groups(&f.f, &ids);
                calls(&r, &expected);
            }
            _ => {
                assert_eq!(f.state(), state);
                calls(&r, &expected);
            }
        }
        if p == "revisited" {
            w.finish();
            return;
        }
        w.reply("ok");
    }
}
pub(super) fn recovers() {
    for mode in [
        "--order-conflict",
        "--order-commit",
        "--order-demoted",
        "--order-denied",
        "--order-session",
        "--order-ack",
        "--order-refresh",
        "--order-member",
        "--order-viewer",
    ] {
        let f = native_group_order::fixture();
        if mode.contains("member") {
            f.role("member");
        }
        if mode.contains("viewer") {
            f.role("viewer");
        }
        let r = Relay::start(f.selection.host.port);
        let mut state = f.state();
        let mut w = launch(&f, &r, mode);
        w.expect("opened");
        assert_eq!(f.state(), state);
        w.reply("ok");
        if mode.contains("member") || mode.contains("viewer") {
            w.expect("disabled");
            calls(&r, &[]);
            assert_eq!(f.state(), state);
            w.finish();
            continue;
        }
        w.expect("ready");
        if mode.contains("conflict") {
            f.client()
                .reorder_main_table_groups(SELECTED, 7, &ACB.map(String::from))
                .unwrap();
            state = state.reordered_groups(&f.f, &ACB);
        } else if mode.contains("commit") {
            f.f.connect()
                .batch_execute(super::group_edit::FAULT)
                .unwrap();
        } else if mode.contains("demoted") {
            f.role("member");
        } else if mode.contains("denied") {
            f.selection.membership(&f.f, false);
        } else if mode.contains("session") {
            f.selection.session.revoke(&f.f);
        } else {
            r.fault
                .store(if mode.contains("ack") { 2 } else { 1 }, SeqCst);
        }
        w.reply("ok");
        let denied =
            mode.contains("demoted") || mode.contains("denied") || mode.contains("session");
        w.expect(if denied { "denied" } else { "failed" });
        let status = if mode.contains("conflict") {
            409
        } else if mode.contains("commit") {
            500
        } else if mode.contains("session") {
            401
        } else if denied {
            403
        } else {
            200
        };
        let mut expected = vec![(status, 7, BAC)];
        calls(&r, &expected);
        if status == 200 {
            state = state.reordered_groups(&f.f, &BAC);
        } else {
            assert_eq!(f.state(), state);
        }
        if denied {
            w.finish();
            continue;
        }
        if mode.contains("commit") {
            f.f.connect()
                .batch_execute("DROP TRIGGER refuse_edit ON board_groups")
                .unwrap();
        }
        if mode.contains("ack") || mode.contains("refresh") {
            let reads = r.reads.load(SeqCst);
            r.fault.store(1, SeqCst);
            w.reply("ok");
            w.expect("read-refused");
            assert_eq!(f.state(), state);
            calls(&r, &expected);
            assert_eq!(r.reads.load(SeqCst), reads + 1);
        }
        w.reply("ok");
        w.expect("reloaded");
        assert_eq!(f.state(), state);
        calls(&r, &expected);
        if mode.contains("conflict") || mode.contains("commit") {
            w.reply("ok");
            w.expect("deliberate");
            let ids = if mode.contains("conflict") { ABC } else { BAC };
            expected.push((200, if mode.contains("conflict") { 8 } else { 7 }, ids));
            state.reordered_groups(&f.f, &ids);
            calls(&r, &expected);
        }
        w.finish();
    }
}
