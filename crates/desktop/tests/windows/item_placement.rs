use super::{
    Window, http_window_command, main_table_relay::Relay, native_placement,
    native_selection::NativeSelection, selection::SELECTED,
};
use boardcraft_mobile_edit_client::NameEdit;
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
fn calls(r: &Relay, reorder: bool, expected: &[(u16, i64, i64, bool)]) {
    let entries = r.entries.lock().unwrap();
    assert_eq!(r.saves.load(SeqCst), expected.len());
    assert_eq!(entries.len(), expected.len());
    let mut operations = std::collections::HashSet::new();
    for (call, &(status, board_revision, item_revision, after)) in entries.iter().zip(expected) {
        assert_eq!(call.status, status);
        let id = Uuid::from_u128(if reorder { 67 } else { 61 });
        assert_eq!(call.method, if reorder { "POST" } else { "PUT" });
        assert_eq!(
            call.path,
            format!(
                "/navigation/boards/{SELECTED}/main-table/items/{id}/{}",
                if reorder { "reorder" } else { "group" }
            )
        );
        if reorder {
            let op = Uuid::parse_str(call.body["operation_id"].as_str().unwrap()).unwrap();
            assert!(!op.is_nil() && operations.insert(op));
            assert_eq!(
                call.body,
                serde_json::json!({"operation_id":op,"group_id":"g-90","anchor_id":Uuid::from_u128(61),"side":if after {"after"} else {"before"},"board_revision":board_revision,"item_revision":item_revision})
            );
        } else {
            assert_eq!(
                call.body,
                serde_json::json!({"group_id":"g-20","expected_revision":item_revision})
            );
        }
    }
}

pub(super) fn moves() {
    for reorder in [false, true] {
        let f = native_placement::fixture(reorder);
        let r = Relay::start(f.selection.host.port);
        let mut state = f.state();
        let mut w = launch(
            &f,
            &r,
            if reorder {
                "--placement-order"
            } else {
                "--placement-move"
            },
        );
        let mut expected = Vec::new();
        let mut lock = f.f.connect();
        let phases: &[&str] = if reorder {
            &[
                "opened",
                "cancelled",
                "invalid",
                "ready",
                "pending",
                "saved",
                "after",
                "revisited",
            ]
        } else {
            &[
                "opened",
                "cancelled",
                "ready",
                "pending",
                "saved",
                "revisited",
            ]
        };
        for &phase in phases {
            w.expect(phase);
            match phase {
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
                "saved" | "after" => {
                    let after = phase == "after";
                    let revision = expected.len() as i64 + 1;
                    expected.push((200, revision, revision, after));
                    state = if reorder {
                        state.reordered_root(&f.f, f.member, after)
                    } else {
                        state.moved_root(&f.f, 61, "g-20")
                    };
                    calls(&r, reorder, &expected);
                    let read = f.client().read_main_table(SELECTED).unwrap();
                    assert_eq!(
                        read.groups()[0]
                            .items()
                            .iter()
                            .map(|r| r.id.as_u128())
                            .collect::<Vec<_>>(),
                        if reorder {
                            if after {
                                vec![73, 61, 67, 77]
                            } else {
                                vec![73, 67, 61, 77]
                            }
                        } else {
                            vec![73]
                        }
                    );
                    assert_eq!(
                        read.groups()[2]
                            .items()
                            .iter()
                            .map(|r| r.id.as_u128())
                            .collect::<Vec<_>>(),
                        if reorder { vec![59] } else { vec![61, 59] }
                    );
                }
                _ => {
                    assert_eq!(f.state(), state);
                    calls(&r, reorder, &expected);
                }
            }
            f.check_access("member", true, true);
            if phase == "revisited" {
                w.finish();
                break;
            }
            w.reply("ok");
        }
        let reopened = launch(
            &f,
            &r,
            if reorder {
                "--placement-order-reopened"
            } else {
                "--placement-move-reopened"
            },
        );
        reopened.expect("revisited");
        assert_eq!(f.state(), state);
        calls(&r, reorder, &expected);
        reopened.finish();
    }
}

pub(super) fn recovers() {
    for reorder in [false, true] {
        for fault in [
            "conflict", "commit", "demoted", "denied", "session", "ack", "refresh",
        ] {
            eprintln!("placement recovery: reorder={reorder} fault={fault}");
            let f = native_placement::fixture(reorder);
            let r = Relay::start(f.selection.host.port);
            let mode = format!(
                "--placement-{}-{fault}",
                if reorder { "order" } else { "move" }
            );
            let mut w = launch(&f, &r, &mode);
            let mut state = f.state();
            w.expect("opened");
            assert_eq!(f.state(), state);
            calls(&r, reorder, &[]);
            w.reply("ok");
            w.expect("ready");
            if fault == "conflict" {
                if reorder {
                    f.client()
                        .rename_main_table_item(
                            SELECTED,
                            Uuid::from_u128(67),
                            &NameEdit {
                                revision: 1,
                                title: "Indigo".into(),
                            },
                        )
                        .unwrap();
                    state = f.state();
                } else {
                    f.client()
                        .move_main_table_item(SELECTED, Uuid::from_u128(61), "g-10", 1)
                        .unwrap();
                    state = state.moved_root(&f.f, 61, "g-10");
                }
            } else if fault == "commit" {
                f.f.connect()
                    .batch_execute(if reorder {
                        native_placement::ORDER_FAULT
                    } else {
                        native_placement::MOVE_FAULT
                    })
                    .unwrap();
            } else if fault == "demoted" {
                f.role("viewer");
            } else if fault == "denied" {
                f.selection.membership(&f.f, false);
            } else if fault == "session" {
                f.selection.session.revoke(&f.f);
            } else {
                r.fault.store(if fault == "ack" { 2 } else { 1 }, SeqCst);
            }
            w.reply("ok");
            let denied = ["demoted", "denied", "session"].contains(&fault);
            w.expect(if denied { "denied" } else { "failed" });
            let status = match fault {
                "conflict" => 409,
                "commit" => 500,
                "demoted" | "denied" => 403,
                "session" => 401,
                _ => 200,
            };
            let mut expected = vec![(status, 1, 1, false)];
            calls(&r, reorder, &expected);
            if status == 200 {
                state = if reorder {
                    state.reordered_root(&f.f, f.member, false)
                } else {
                    state.moved_root(&f.f, 61, "g-20")
                };
            } else {
                assert_eq!(f.state(), state);
            }
            if fault == "denied" || fault == "session" {
                w.finish();
                continue;
            }
            if fault == "commit" {
                f.f.connect()
                    .batch_execute(if reorder {
                        "DROP TRIGGER refuse_placement ON group_reorder_receipts"
                    } else {
                        "DROP TRIGGER refuse_placement ON board_items"
                    })
                    .unwrap();
            }
            if fault == "demoted" {
                f.role("member");
            }
            let reads = r.reads.load(SeqCst);
            if fault == "ack" || fault == "refresh" {
                r.fault.store(1, SeqCst);
            }
            w.reply("ok");
            if fault == "ack" || fault == "refresh" {
                w.expect("read-refused");
                assert_eq!(f.state(), state);
                calls(&r, reorder, &expected);
                assert_eq!(r.reads.load(SeqCst), reads + 1);
                w.reply("ok");
            }
            w.expect("reloaded");
            assert_eq!(f.state(), state);
            calls(&r, reorder, &expected);
            assert_eq!(
                r.reads.load(SeqCst),
                reads
                    + if fault == "ack" || fault == "refresh" {
                        2
                    } else {
                        1
                    }
            );
            w.reply("ok");
            w.expect("deliberate");
            let item_revision = if ["conflict", "ack", "refresh"].contains(&fault) {
                2
            } else {
                1
            };
            let board_revision = if reorder && ["ack", "refresh"].contains(&fault) {
                2
            } else {
                1
            };
            let after = reorder && ["ack", "refresh"].contains(&fault);
            expected.push((200, board_revision, item_revision, after));
            state = if reorder {
                state.reordered_root(&f.f, f.member, after)
            } else {
                state.moved_root(&f.f, 61, "g-20")
            };
            assert_eq!(f.state(), state);
            calls(&r, reorder, &expected);
            w.finish();
        }
    }
}
