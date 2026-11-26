use super::main_table_relay;
use super::{
    Window, http_window_command,
    native_table::{A, B, C, NativeTable},
    selection::SELECTED,
};
use std::sync::atomic::Ordering::SeqCst;

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

pub(super) fn folds() {
    let f = NativeTable::new();
    let mut state = f.state();
    let mut w = launch(&f, f.selection.host.port, "--table-member");
    for (checkpoint, rev, ids) in [
        ("opened", 1, &[][..]),
        ("folded-b", 2, &[B][..]),
        ("collapsed-all", 3, &[A, B, C][..]),
        ("expanded-all", 4, &[][..]),
        ("folded-empty", 5, &[C][..]),
        ("menu-collapsed", 6, &[B, C][..]),
        ("menu-expanded", 7, &[C][..]),
        ("shared", 8, &[B, C][..]),
    ] {
        w.expect(checkpoint);
        if rev > 1 {
            state = f.folded(&state, rev, ids);
        } else {
            assert_eq!(f.state(), state);
        }
        f.check_read(rev, ids, true);
        if checkpoint == "shared" {
            w.finish();
            break;
        }
        if checkpoint == "opened" {
            let mut lock = f.f.connect();
            lock.batch_execute("BEGIN").unwrap();
            lock.query_one("SELECT id FROM boards WHERE id=$1 FOR UPDATE", &[&SELECTED])
                .unwrap();
            w.reply("ok");
            w.expect("pending");
            assert_eq!(f.state(), state);
            lock.batch_execute("COMMIT").unwrap();
            w.reply("ok");
        } else {
            w.reply("ok");
        }
    }
    let w = launch(&f, f.selection.host.port, "--table-reopen");
    w.expect("reopened");
    assert_eq!(f.state(), state);
    w.finish();
    f.role("viewer");
    let mut w = launch(&f, f.selection.host.port, "--table-viewer");
    for checkpoint in [
        "viewer-opened",
        "viewer-expanded",
        "viewer-all",
        "viewer-open",
        "viewer-revisited",
    ] {
        w.expect(checkpoint);
        assert_eq!(f.state(), state);
        f.check_read(8, &[B, C], false);
        w.reply("ok");
    }
    super::wait_for_close(&mut w.0);
    let w = launch(&f, f.selection.host.port, "--table-reopen");
    w.expect("reopened");
    assert_eq!(f.state(), state);
    w.finish();
}

pub(super) fn recovers() {
    for mode in [
        "--table-conflict",
        "--table-refresh-fault",
        "--table-ack-fault",
        "--table-commit-fault",
        "--table-demoted",
    ] {
        let f = NativeTable::new();
        let relay = main_table_relay::Relay::start(f.selection.host.port);
        let mut state = f.state();
        let mut w = launch(&f, relay.port, mode);
        w.expect("rendered");
        assert_eq!(f.state(), state);
        match mode {
            "--table-conflict" => {
                f.client().set_main_table_fold(SELECTED,1,&[C.to_owned()],true).unwrap();
                state=f.folded(&state,2,&[C]);
            }
            "--table-refresh-fault" => relay.fault.store(1,SeqCst),
            "--table-ack-fault" => relay.fault.store(2,SeqCst),
            "--table-commit-fault" => f.f.connect().batch_execute("CREATE FUNCTION refuse_fold() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture refusal'; END $$; CREATE CONSTRAINT TRIGGER refuse_fold AFTER INSERT ON main_tables DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION refuse_fold()").unwrap(),
            "--table-demoted" => f.role("viewer"),
            _ => unreachable!(),
        }
        w.reply("ok");
        w.expect("refused");
        assert_eq!(relay.saves.load(SeqCst), 1);
        if matches!(mode, "--table-refresh-fault" | "--table-ack-fault") {
            state = f.folded(&state, 2, &[B]);
        } else {
            assert_eq!(f.state(), state);
        }
        if mode == "--table-commit-fault" {
            f.f.connect()
                .batch_execute("DROP TRIGGER refuse_fold ON main_tables")
                .unwrap();
        }
        w.reply("ok");
        w.expect("reviewed");
        assert_eq!(relay.saves.load(SeqCst), 1);
        assert_eq!(f.state(), state);
        w.reply("ok");
        if mode == "--table-demoted" {
            w.expect("temporary");
            assert_eq!(relay.saves.load(SeqCst), 1);
            assert_eq!(f.state(), state);
            w.reply("ok");
            w.expect("discarded");
            assert_eq!(f.state(), state);
            w.finish();
        } else {
            w.expect("deliberate");
            assert_eq!(relay.saves.load(SeqCst), 2);
            let (rev, ids) = if mode == "--table-commit-fault" {
                (2, &[B][..])
            } else {
                (3, &[B, C][..])
            };
            state = f.folded(&state, rev, ids);
            if mode == "--table-conflict" {
                f.selection.membership(&f.f, false);
                w.reply("ok");
                w.expect("access-denied");
                assert_eq!(f.state(), state);
                assert_eq!(relay.saves.load(SeqCst), 2);
            }
            w.finish();
        }
    }
}
