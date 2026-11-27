use super::{
    Window, http_window_command, main_table_relay::Relay, native_group_fields,
    native_selection::NativeSelection, selection::SELECTED,
};
use boardcraft_mobile_edit_client::GroupFields;
use std::sync::atomic::Ordering::SeqCst;

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
#[derive(Clone, Copy)]
enum Target {
    Create,
    Existing,
    Created,
}
type Expected<'a> = (Target, u16, i64, Option<i64>, &'a str, &'a str);
fn calls(r: &Relay, expected: &[Expected<'_>]) -> Option<String> {
    let entries = r.entries.lock().unwrap();
    assert_eq!(entries.len(), expected.len());
    assert_eq!(r.saves.load(SeqCst), expected.len());
    let mut created: Option<String> = None;
    for (e, &(target, status, board, group, name, colour)) in entries.iter().zip(expected) {
        let create = matches!(target, Target::Create);
        assert_eq!(e.method, if create { "POST" } else { "PUT" });
        let base = format!("/navigation/boards/{SELECTED}/main-table/groups");
        assert_eq!(
            e.path,
            if create {
                base
            } else {
                let id = match target {
                    Target::Created => created.as_deref().expect("earlier create"),
                    _ => "g-z",
                };
                let encoded: String = id.bytes().map(|b| format!("%{b:02X}")).collect();
                format!("{base}/{encoded}")
            }
        );
        assert_eq!(e.status, status);
        assert_eq!(e.body["board_revision"], board);
        assert_eq!(e.body["name"], name);
        assert_eq!(e.body["colour"], colour);
        if let Some(rev) = group {
            assert_eq!(e.body["group_revision"], rev);
        } else {
            let id = e.body["id"].as_str().unwrap();
            assert!(
                !uuid::Uuid::parse_str(id.strip_prefix("g-").unwrap())
                    .unwrap()
                    .is_nil()
            );
            if let Some(prior) = &created {
                assert_eq!(prior, id);
            } else {
                created = Some(id.to_owned());
            }
        }
    }
    created
}
pub(super) fn creates_and_edits() {
    let f = native_group_fields::fixture();
    let r = Relay::start(f.selection.host.port);
    let mut state = f.state();
    let mut w = launch(&f, &r, "--groups-fields");
    let mut expected = Vec::new();
    let mut lock = f.f.connect();
    for point in [
        "opened",
        "cancel-ready",
        "cancelled",
        "draft",
        "pending",
        "created",
        "edit-cancelled",
        "edit-confirming",
        "edit-pending",
        "unchanged",
        "colour-cancelled",
        "edited",
        "name-only",
        "colour-only",
        "created-edited",
        "revisited",
    ] {
        w.expect(point);
        match point {
            "created" => {
                expected.push((
                    Target::Create,
                    200,
                    7,
                    None,
                    "  Work  ",
                    super::group_create::GREEN,
                ));
                let id = calls(&r, &expected).unwrap();
                state = state.created_group(&f.f, &id, "Work", "#00c875", 3);
            }
            "unchanged" => {
                expected.push((Target::Existing, 200, 8, Some(1), "Work", "#443322"));
                state = state.edited_group(&f.f, "g-z", "Work", "#443322");
            }
            "edited" => {
                expected.push((Target::Existing, 200, 9, Some(2), "  Reserve  ", "#00c875"));
                state = state.edited_group(&f.f, "g-z", "Reserve", "#00c875");
            }
            "name-only" => {
                expected.push((Target::Existing, 200, 10, Some(3), "  Ink  ", "#00c875"));
                state = state.edited_group(&f.f, "g-z", "Ink", "#00c875");
            }
            "colour-only" => {
                expected.push((
                    Target::Existing,
                    200,
                    11,
                    Some(4),
                    "Ink",
                    super::group_create::BLUE,
                ));
                state = state.edited_group(&f.f, "g-z", "Ink", "#579bfc");
            }
            "created-edited" => {
                expected.push((Target::Created, 200, 12, Some(1), "  Reserve  ", "#579bfc"));
                let id = calls(&r, &expected).unwrap();
                state = state.edited_group(&f.f, &id, "Reserve", "#579bfc");
            }
            _ => assert_eq!(f.state(), state),
        }
        f.check_access("owner", true, true);
        if matches!(point, "draft" | "edit-confirming") {
            lock.batch_execute("BEGIN").unwrap();
            lock.query_one("SELECT id FROM boards WHERE id=$1 FOR UPDATE", &[&SELECTED])
                .unwrap();
        }
        if matches!(point, "pending" | "edit-pending") {
            super::grouped_entry::wait_for_write(&mut lock);
            assert_eq!(r.saves.load(SeqCst), if point == "pending" { 1 } else { 2 });
            lock.batch_execute("COMMIT").unwrap();
        } else {
            calls(&r, &expected);
        }
        if point == "revisited" {
            w.finish();
            return;
        }
        w.reply("ok");
    }
}
pub(super) fn recovers() {
    for mode in [
        "--groups-edit-conflict",
        "--groups-create-conflict",
        "--groups-edit-demoted",
        "--groups-create-demoted",
        "--groups-edit-ack",
        "--groups-create-ack",
        "--groups-create-absent",
        "--groups-edit-refresh",
        "--groups-create-refresh",
        "--groups-edit-missing",
        "--groups-edit-denied",
        "--groups-create-session",
        "--groups-member",
        "--groups-viewer",
    ] {
        let f = native_group_fields::fixture();
        let c = f.client();
        if mode == "--groups-member" {
            f.role("member");
        }
        if mode == "--groups-viewer" {
            f.role("viewer");
        }
        let r = Relay::start(f.selection.host.port);
        let mut state = f.state();
        let mut w = launch(&f, &r, mode);
        w.expect("opened");
        assert_eq!(f.state(), state);
        w.reply("ok");
        if mode == "--groups-member" || mode == "--groups-viewer" {
            w.expect("disabled");
            calls(&r, &[]);
            assert_eq!(f.state(), state);
            w.finish();
            continue;
        }
        let create = mode.contains("create");
        let target = if create {
            Target::Create
        } else {
            Target::Existing
        };
        if create {
            for point in ["cancel-ready", "cancelled"] {
                w.expect(point);
                assert_eq!(f.state(), state);
                calls(&r, &[]);
                w.reply("ok");
            }
        }
        w.expect("draft");
        assert_eq!(f.state(), state);
        calls(&r, &[]);
        if mode.contains("conflict") {
            f.f.connect()
                .batch_execute(if create {
                    super::group_create::FAULT
                } else {
                    super::group_edit::FAULT
                })
                .unwrap();
        } else if mode.contains("demoted") {
            f.role("member");
        } else if mode.contains("denied") {
            f.selection.membership(&f.f, false);
        } else if mode.contains("session") {
            f.selection.session.revoke(&f.f);
        } else if mode.contains("missing") {
            c.archive_main_table_group(SELECTED, "g-z", 7, false)
                .unwrap();
            state = state.group_transition(&f.f, "g-z", false);
        } else {
            r.fault.store(
                if mode.contains("ack") {
                    2
                } else if mode.contains("absent") {
                    3
                } else {
                    1
                },
                SeqCst,
            );
        }
        w.reply("ok");
        let rev = if create { None } else { Some(1) };
        if mode.contains("conflict") {
            w.expect("refused");
            assert_eq!(f.state(), state);
            let expected = [(target, 500, 7, rev, "  Work  ", "#00c875")];
            calls(&r, &expected);
            f.f.connect()
                .batch_execute(if create {
                    "DROP TRIGGER refuse_group ON board_groups"
                } else {
                    "DROP TRIGGER refuse_edit ON board_groups"
                })
                .unwrap();
            if create {
                let peer = format!("g-{}", uuid::Uuid::new_v4());
                c.create_main_table_group(SELECTED, &peer, &GroupFields::new(7, "Peer", "#abcdef"))
                    .unwrap();
                state = state.created_group(&f.f, &peer, "Peer", "#abcdef", 3);
            } else {
                c.edit_main_table_group(
                    SELECTED,
                    "g-z",
                    1,
                    &GroupFields::new(7, "Peer", "#abcdef"),
                )
                .unwrap();
                state = state.edited_group(&f.f, "g-z", "Peer", "#abcdef");
            }
            w.reply("ok");
            let mut expected = expected.to_vec();
            expected.push((target, 409, 7, rev, "  Work  ", "#00c875"));
            for point in ["conflict", "reviewed", "adopted"] {
                w.expect(point);
                assert_eq!(f.state(), state);
                calls(&r, &expected);
                f.check_access("owner", true, true);
                w.reply("ok");
            }
            w.expect("deliberate");
            expected.push((
                target,
                200,
                8,
                if create { None } else { Some(2) },
                "  Work  ",
                "#00c875",
            ));
            let id = calls(&r, &expected);
            if create {
                state.created_group(&f.f, &id.unwrap(), "Work", "#00c875", 4);
            } else {
                state.edited_group(&f.f, "g-z", "Work", "#00c875");
            }
            w.finish();
        } else if mode.contains("missing") {
            for point in ["conflict", "missing"] {
                w.expect(point);
                assert_eq!(f.state(), state);
                calls(
                    &r,
                    &[(Target::Existing, 409, 7, Some(1), "  Work  ", "#00c875")],
                );
                if point == "missing" {
                    w.finish();
                    break;
                }
                w.reply("ok");
            }
        } else if mode.contains("demoted") || mode.contains("denied") || mode.contains("session") {
            w.expect("denied");
            assert_eq!(f.state(), state);
            calls(
                &r,
                &[(
                    target,
                    if mode.contains("session") { 401 } else { 403 },
                    7,
                    rev,
                    "  Work  ",
                    "#00c875",
                )],
            );
            f.check_access(
                if mode.contains("demoted") {
                    "member"
                } else {
                    "owner"
                },
                !mode.contains("denied"),
                !mode.contains("session"),
            );
            w.finish();
        } else {
            w.expect("uncertain");
            let expected = [(
                target,
                if mode.contains("absent") { 503 } else { 200 },
                7,
                rev,
                "  Work  ",
                "#00c875",
            )];
            let id = calls(&r, &expected);
            state = if mode.contains("absent") {
                assert_eq!(f.state(), state);
                state
            } else if create {
                state.created_group(&f.f, &id.unwrap(), "Work", "#00c875", 3)
            } else {
                state.edited_group(&f.f, "g-z", "Work", "#00c875")
            };
            let reads = r.reads.load(SeqCst);
            r.fault.store(1, SeqCst);
            w.reply("ok");
            w.expect("reload-refused");
            assert_eq!(f.state(), state);
            calls(&r, &expected);
            assert_eq!(r.reads.load(SeqCst), reads + 1);
            w.reply("ok");
            if mode.contains("absent") {
                for point in ["reviewed", "adopted"] {
                    w.expect(point);
                    assert_eq!(f.state(), state);
                    calls(&r, &expected);
                    w.reply("ok");
                }
                w.expect("deliberate");
                let mut expected = expected.to_vec();
                expected.push((Target::Create, 200, 7, None, "  Work  ", "#00c875"));
                let id = calls(&r, &expected).unwrap();
                state.created_group(&f.f, &id, "Work", "#00c875", 3);
                w.finish();
                continue;
            }
            w.expect("reconciled");
            assert_eq!(f.state(), state);
            calls(&r, &expected);
            assert_eq!(r.reads.load(SeqCst), reads + 2);
            w.finish();
        }
    }
}
