use super::{
    Window, grouped_entry_fixture as oracle, http_window_command, main_table_relay as relay,
    native_table::{B, C, NativeTable},
    selection::SELECTED,
};
use std::sync::atomic::Ordering::SeqCst;
use uuid::Uuid;

pub(super) fn wait_for_write(lock: &mut postgres::Client) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !lock.query_one("SELECT EXISTS (SELECT FROM pg_locks WHERE NOT granted AND pg_backend_pid() = ANY(pg_blocking_pids(pid)))", &[]).unwrap().get::<_, bool>(0) {
        assert!(std::time::Instant::now() < until, "entry did not reach the fixture lock");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn fixture() -> NativeTable {
    let f = NativeTable::new();
    f.client()
        .set_main_table_fold(SELECTED, 1, &[C.to_owned()], true)
        .unwrap();
    f
}

fn launch(f: &NativeTable, port: u16, mode: &str) -> Window {
    let mut cmd = http_window_command();
    cmd.args(["browse", &port.to_string(), mode])
        .env(
            "BOARDCRAFT_SESSION_ORIGIN",
            format!("http://127.0.0.1:{port}"),
        )
        .env("BOARDCRAFT_SESSION_TOKEN", f.selection.token.encoded());
    Window::launch(cmd)
}

fn proposed(relay: &relay::Relay) -> Uuid {
    serde_json::from_value(relay.entries.lock().unwrap()[0].body["id"].clone()).unwrap()
}

fn calls(relay: &relay::Relay, expected: &[(&str, &str, u16)], id: Option<Uuid>) {
    let entries = relay.entries.lock().unwrap();
    assert_eq!(entries.len(), expected.len());
    for (call, &(method, title, status)) in entries.iter().zip(expected) {
        assert_eq!(call.method, method);
        assert_eq!(call.body["title"], title);
        assert_eq!(call.status, status);
        if method == "POST" {
            assert_eq!(
                call.path,
                format!("/navigation/boards/{SELECTED}/main-table/items")
            );
            assert_eq!(call.body["group_id"], B);
            if let Some(id) = id {
                assert_eq!(call.body["id"], serde_json::json!(id));
            }
        } else {
            let id = id.unwrap_or_else(|| Uuid::from_u128(61));
            assert_eq!(
                call.path,
                format!("/navigation/boards/{SELECTED}/main-table/items/{id}/name")
            );
            assert_eq!(call.body["revision"], 1);
        }
    }
}

pub(super) fn creates_and_renames() {
    let f = fixture();
    let before = f.state();
    let relay = relay::Relay::start(f.selection.host.port);
    let mut w = launch(&f, relay.port, "--entry-member");
    let mut lock = f.f.connect();
    let mut id = None;
    let long = "😀".repeat(128);
    for checkpoint in [
        "opened",
        "cancelled",
        "rejected",
        "corrected",
        "pending",
        "created",
        "name-cancelled",
        "unchanged",
        "name-draft",
        "name-pending",
        "renamed",
        "revisited",
    ] {
        w.expect(checkpoint);
        if matches!(checkpoint, "pending" | "name-pending") {
            wait_for_write(&mut lock);
        }
        let saved = match checkpoint {
            "created" | "name-cancelled" | "unchanged" | "name-draft" | "name-pending" => {
                Some((id.unwrap(), "Ink", 1))
            }
            "renamed" | "revisited" => Some((id.unwrap(), "Coral", 2)),
            _ => None,
        };
        if matches!(checkpoint, "pending" | "name-pending") {
            oracle::check_storage(&f, &before, saved);
        } else {
            oracle::check(&f, &before, saved);
        }
        assert!(f.selection.session.present(&f.f));
        assert_eq!(
            relay.reads.load(SeqCst),
            match checkpoint {
                "created" | "name-cancelled" | "unchanged" | "name-draft" | "name-pending" => 2,
                "renamed" => 3,
                "revisited" => 4,
                _ => 1,
            }
        );
        match checkpoint {
            "opened" | "cancelled" => assert_eq!(relay.saves.load(SeqCst), 0),
            "rejected" => {
                id = Some(proposed(&relay));
                assert!(!id.unwrap().is_nil());
                calls(&relay, &[("POST", &long, 422)], id);
            }
            "corrected" => {
                calls(&relay, &[("POST", &long, 422)], id);
                lock.batch_execute("BEGIN").unwrap();
                lock.query_one(
                    "SELECT id FROM board_groups WHERE board_id=$1 AND id=$2 FOR UPDATE",
                    &[&SELECTED, &B],
                )
                .unwrap();
            }
            "pending" => {
                assert_eq!(relay.saves.load(SeqCst), 2);
                calls(&relay, &[("POST", &long, 422)], id);
                lock.batch_execute("COMMIT").unwrap();
            }
            "name-draft" => {
                calls(&relay, &[("POST", &long, 422), ("POST", " Ink ", 200)], id);
                lock.batch_execute("BEGIN").unwrap();
                lock.query_one(
                    "SELECT id FROM board_items WHERE id=$1 FOR UPDATE",
                    &[&id.unwrap()],
                )
                .unwrap();
            }
            "name-pending" => {
                assert_eq!(relay.saves.load(SeqCst), 3);
                lock.batch_execute("COMMIT").unwrap();
            }
            "renamed" | "revisited" => calls(
                &relay,
                &[
                    ("POST", &long, 422),
                    ("POST", " Ink ", 200),
                    ("PUT", " Coral ", 200),
                ],
                id,
            ),
            _ => calls(&relay, &[("POST", &long, 422), ("POST", " Ink ", 200)], id),
        }
        if checkpoint == "revisited" {
            w.finish();
            return;
        }
        w.reply("ok");
    }
}

pub(super) fn recovers() {
    for mode in [
        "--entry-demoted-create",
        "--entry-demoted-name",
        "--entry-name-validation",
        "--entry-ack-create",
        "--entry-refresh-create",
        "--entry-ack-name",
        "--entry-refresh-name",
        "--entry-revoked-name",
        "--entry-viewer",
    ] {
        let f = fixture();
        let before = f.state();
        let relay = relay::Relay::start(f.selection.host.port);
        if mode == "--entry-viewer" {
            f.role("viewer");
            let w = launch(&f, relay.port, mode);
            w.expect("viewer");
            oracle::check(&f, &before, None);
            assert_eq!(relay.saves.load(SeqCst), 0);
            assert_eq!(relay.reads.load(SeqCst), 1);
            w.finish();
            continue;
        }
        let mut w = launch(&f, relay.port, mode);
        w.expect("draft");
        oracle::check(&f, &before, None);
        assert_eq!(relay.saves.load(SeqCst), 0);
        match mode {
            "--entry-demoted-create" | "--entry-demoted-name" => f.role("viewer"),
            "--entry-revoked-name" => f.selection.session.revoke(&f.f),
            _ if mode.starts_with("--entry-ack") => relay.fault.store(2, SeqCst),
            _ if mode.starts_with("--entry-refresh") => relay.fault.store(1, SeqCst),
            _ => (),
        }
        w.reply("ok");
        let method = if mode.ends_with("-name") || mode == "--entry-name-validation" {
            "PUT"
        } else {
            "POST"
        };
        if mode == "--entry-revoked-name" {
            w.expect("expired");
            calls(&relay, &[(method, " Coral ", 401)], None);
            assert_eq!(relay.reads.load(SeqCst), 1);
            oracle::check_storage(&f, &before, None);
            assert!(!f.selection.session.present(&f.f));
            w.finish();
        } else if mode.starts_with("--entry-demoted") {
            w.expect("denied");
            calls(
                &relay,
                &[(
                    method,
                    if method == "PUT" { " Coral " } else { " Ink " },
                    403,
                )],
                None,
            );
            oracle::check(&f, &before, None);
            assert_eq!(relay.reads.load(SeqCst), 2);
            w.reply("ok");
            w.expect("viewer");
            oracle::check(&f, &before, None);
            assert_eq!(relay.saves.load(SeqCst), 1);
            w.finish();
        } else if mode == "--entry-name-validation" {
            for checkpoint in ["name-rejected", "name-corrected", "name-corrected-saved"] {
                w.expect(checkpoint);
                if checkpoint == "name-corrected-saved" {
                    assert_eq!(relay.reads.load(SeqCst), 2);
                    oracle::check_name(&f, &before, "Coral", 2);
                    calls(
                        &relay,
                        &[("PUT", "   ", 422), ("PUT", " Coral ", 200)],
                        None,
                    );
                    w.finish();
                    break;
                }
                oracle::check(&f, &before, None);
                assert_eq!(relay.reads.load(SeqCst), 1);
                calls(&relay, &[("PUT", "   ", 422)], None);
                w.reply("ok");
            }
        } else {
            w.expect("unconfirmed");
            let id = if method == "POST" {
                Some(proposed(&relay))
            } else {
                None
            };
            let check = || {
                assert_eq!(relay.saves.load(SeqCst), 1);
                calls(
                    &relay,
                    &[(
                        method,
                        if method == "PUT" { " Coral " } else { " Ink " },
                        200,
                    )],
                    id,
                );
                if let Some(id) = id {
                    oracle::check(&f, &before, Some((id, "Ink", 1)));
                } else {
                    oracle::check_name(&f, &before, "Coral", 2);
                }
            };
            check();
            let reads = if mode.starts_with("--entry-refresh") {
                2
            } else {
                1
            };
            assert_eq!(relay.reads.load(SeqCst), reads);
            w.reply("ok");
            w.expect("reconciled");
            check();
            assert_eq!(relay.reads.load(SeqCst), reads + 1);
            w.finish();
        }
    }
}
