use crate::{grouped_board, lifecycle, reorder_state::Snapshot, selection::SELECTED};
use boardcraft_account::Role;
use uuid::Uuid;

pub fn run() {
    lifecycle(false);
}
pub fn run_trash() {
    lifecycle(true);
}
fn lifecycle(trash: bool) {
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
        let selected = if trash {
            &[67, 59][..]
        } else {
            &[67, 59, 61][..]
        };
        let peer_target = if trash { 59 } else { 61 };
        let action = |step: &str| format!("{} {step}", if trash { "trash" } else { "bulk" });
        let accepted = |state: &Snapshot, f: &crate::fixture::Fixture, actor, op| {
            if trash {
                state.deleted_selection(f, actor, op, selected)
            } else {
                state.archived_selection(f, actor, op, selected)
            }
        };
        let mut state = Snapshot::read(&app.f);
        app.act(if mode == "viewer" {
            "select viewer"
        } else {
            "select member"
        });
        assert_eq!(Snapshot::read(&app.f), state);
        if matches!(mode, "member" | "viewer") {
            app.act(&action("readonly"));
            assert_eq!(Snapshot::read(&app.f), state);
            continue;
        }
        app.act(&action("prepare"));
        state = state.folded(&app.f, 2, &["g-20"]);
        app.act(&action(if mode == "success" { "cancel" } else { "open" }));
        assert_eq!(Snapshot::read(&app.f), state);
        match mode {
            "success" => {
                let mut db = app.f.connect();
                let mut tx = db.transaction().unwrap();
                tx.query_one("SELECT id FROM boards WHERE id=$1 FOR UPDATE", &[&SELECTED])
                    .unwrap();
                app.act(&action("pending"));
                assert_eq!(Snapshot::read(&app.f), state);
                tx.rollback().unwrap();
                let op = app.proposed_item(&action("saved"));
                state = accepted(&state, &app.f, app.account, op);
                app.act(&action("inspect"));
                assert_eq!(Snapshot::read(&app.f), state);
                app.act(&action("restore"));
                state = state.lifecycle_records(
                    &app.f,
                    app.account,
                    if trash { "restore-deleted" } else { "restore" },
                );
                app.act(&action("reopen"));
                assert_eq!(Snapshot::read(&app.f), state);
            }
            "conflict" | "missing" | "commit" => {
                if mode == "commit" {
                    app.f.connect().batch_execute("CREATE FUNCTION fail_bulk() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER fail_bulk AFTER INSERT ON group_reorder_receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_bulk()").unwrap();
                } else {
                    app.f.connect().execute(if mode=="conflict" {"UPDATE board_items SET title='Indigo',revision=revision+1 WHERE id=$1"} else {"UPDATE board_items SET state=1,revision=revision+1 WHERE id=$1"},&[&Uuid::from_u128(peer_target)]).unwrap();
                    state = Snapshot::read(&app.f);
                }
                app.act(&action(if mode == "commit" {
                    "fault"
                } else {
                    "conflict"
                }));
                assert_eq!(Snapshot::read(&app.f), state);
                if mode == "commit" {
                    app.f
                        .connect()
                        .batch_execute("DROP TRIGGER fail_bulk ON group_reorder_receipts")
                        .unwrap();
                }
                if mode == "conflict" {
                    app.act(&action("failed review"));
                    assert_eq!(Snapshot::read(&app.f), state);
                }
                app.act(&action(if mode == "missing" {
                    "missing review"
                } else if mode == "commit" {
                    "review fault"
                } else {
                    "review"
                }));
                assert_eq!(Snapshot::read(&app.f), state);
                if mode != "missing" {
                    app.act(&action("adopt"));
                    assert_eq!(Snapshot::read(&app.f), state);
                    let op = app.proposed_item(&action(if mode == "commit" {
                        "confirmed fault"
                    } else {
                        "confirmed"
                    }));
                    accepted(&state, &app.f, app.account, op);
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
                app.act(&action("denied"));
                assert_eq!(Snapshot::read(&app.f), state);
            }
            "denied" => {
                app.catalog.membership(&app.f, false);
                app.act(&action("no access"));
                assert_eq!(Snapshot::read(&app.f), state);
                app.catalog.check_grants(&app.f, false);
            }
            "session" => {
                session.revoke(&app.f);
                app.act(&action("expired"));
                assert_eq!(Snapshot::read(&app.f), state);
            }
            _ => {
                let op = app.proposed_item(&action(if mode == "ack" {
                    "uncertain ack"
                } else {
                    "uncertain refresh"
                }));
                state = accepted(&state, &app.f, app.account, op);
                app.act(&action("reconcile"));
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
