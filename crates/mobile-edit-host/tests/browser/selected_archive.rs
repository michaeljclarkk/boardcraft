use crate::{grouped_board, lifecycle, reorder_state::Snapshot, selection::SELECTED};
use boardcraft_account::Role;
use uuid::Uuid;

pub fn run() {
    for mode in [
        "success", "conflict", "commit", "missing", "demoted", "denied", "session", "ack",
        "refresh", "member", "viewer",
    ] {
        let (mut app, session) = grouped_board(if mode == "member" {
            Role::Member
        } else if mode == "viewer" {
            Role::Viewer
        } else {
            Role::Owner
        });
        lifecycle::seed(&app.f, app.account);
        let mut state = Snapshot::read(&app.f);
        app.act(if mode == "viewer" {
            "select viewer"
        } else {
            "select member"
        });
        assert_eq!(Snapshot::read(&app.f), state);
        if matches!(mode, "member" | "viewer") {
            app.act("bulk readonly");
            assert_eq!(Snapshot::read(&app.f), state);
            continue;
        }
        app.act("bulk prepare");
        state = state.folded(&app.f, 2, &["g-20"]);
        app.act(if mode == "success" {
            "bulk cancel"
        } else {
            "bulk open"
        });
        assert_eq!(Snapshot::read(&app.f), state);
        match mode {
            "success" => {
                let mut db = app.f.connect();
                let mut tx = db.transaction().unwrap();
                tx.query_one("SELECT id FROM boards WHERE id=$1 FOR UPDATE", &[&SELECTED])
                    .unwrap();
                app.act("bulk pending");
                assert_eq!(Snapshot::read(&app.f), state);
                tx.rollback().unwrap();
                let op = app.proposed_item("bulk saved");
                state = state.archived_selection(&app.f, app.account, op, &[67, 59, 61]);
                app.act("bulk inspect");
                assert_eq!(Snapshot::read(&app.f), state);
                app.act("bulk restore");
                state = state.lifecycle_records(&app.f, app.account, "restore");
                app.act("bulk reopen");
                assert_eq!(Snapshot::read(&app.f), state);
            }
            "conflict" | "missing" | "commit" => {
                if mode == "commit" {
                    app.f.connect().batch_execute("CREATE FUNCTION fail_bulk() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER fail_bulk AFTER INSERT ON group_reorder_receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_bulk()").unwrap();
                } else {
                    app.f.connect().execute(if mode=="conflict" {"UPDATE board_items SET title='Indigo',revision=revision+1 WHERE id=$1"} else {"UPDATE board_items SET state=1,revision=revision+1 WHERE id=$1"},&[&Uuid::from_u128(61)]).unwrap();
                    state = Snapshot::read(&app.f);
                }
                app.act(if mode == "commit" {
                    "bulk fault"
                } else {
                    "bulk conflict"
                });
                assert_eq!(Snapshot::read(&app.f), state);
                if mode == "commit" {
                    app.f
                        .connect()
                        .batch_execute("DROP TRIGGER fail_bulk ON group_reorder_receipts")
                        .unwrap();
                }
                if mode == "conflict" {
                    app.act("bulk failed review");
                    assert_eq!(Snapshot::read(&app.f), state);
                }
                app.act(if mode == "missing" {
                    "bulk missing review"
                } else if mode == "commit" {
                    "bulk review fault"
                } else {
                    "bulk review"
                });
                assert_eq!(Snapshot::read(&app.f), state);
                if mode != "missing" {
                    app.act("bulk adopt");
                    assert_eq!(Snapshot::read(&app.f), state);
                    let op = app.proposed_item(if mode == "commit" {
                        "bulk confirmed fault"
                    } else {
                        "bulk confirmed"
                    });
                    state.archived_selection(&app.f, app.account, op, &[67, 59, 61]);
                }
            }
            "demoted" => {
                app.f
                    .connect()
                    .execute(
                        "UPDATE accounts SET role='member' WHERE id=$1",
                        &[&app.account],
                    )
                    .unwrap();
                app.act("bulk denied");
                assert_eq!(Snapshot::read(&app.f), state);
            }
            "denied" => {
                app.catalog.membership(&app.f, false);
                app.act("bulk no access");
                assert_eq!(Snapshot::read(&app.f), state);
                app.catalog.check_grants(&app.f, false);
            }
            "session" => {
                session.revoke(&app.f);
                app.act("bulk expired");
                assert_eq!(Snapshot::read(&app.f), state);
            }
            _ => {
                let op = app.proposed_item(if mode == "ack" {
                    "bulk uncertain ack"
                } else {
                    "bulk uncertain refresh"
                });
                state = state.archived_selection(&app.f, app.account, op, &[67, 59, 61]);
                app.act("bulk reconcile");
                assert_eq!(Snapshot::read(&app.f), state);
            }
        }
        if mode != "session" {
            assert!(session.present(&app.f));
        }
        if mode != "denied" {
            app.catalog.check_grants(&app.f, true);
        }
    }
}
