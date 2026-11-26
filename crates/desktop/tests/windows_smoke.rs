//! Real Windows processes; run in a graphical session with --features native-smoke.
#![cfg(windows)]

use boardcraft_board::{Board, BoardId, ItemId, store};
use boardcraft_desktop::database;
use postgres::{Config, NoTls};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[cfg(feature = "attended-clipboard")]
#[path = "windows/board_link.rs"]
mod board_link;
#[path = "../../mobile-edit-host/tests/fixture/mod.rs"]
mod fixture;
#[path = "../../mobile-edit-host/tests/fixture/group_create.rs"]
mod group_create;
#[path = "../../mobile-edit-host/tests/fixture/group_edit.rs"]
mod group_edit;
#[path = "windows/grouped_entry.rs"]
mod grouped_entry;
#[path = "../../mobile-edit-host/tests/fixture/grouped_entry.rs"]
mod grouped_entry_fixture;
#[path = "windows/logout.rs"]
mod logout;
#[path = "windows/main_table.rs"]
mod main_table;
#[path = "windows/main_table_relay.rs"]
mod main_table_relay;
#[path = "../../mobile-edit-host/tests/fixture/native_selection.rs"]
mod native_selection;
#[path = "../../mobile-edit-host/tests/fixture/reorder_state.rs"]
#[allow(
    dead_code,
    reason = "shared oracle includes other browser order workflows"
)]
mod reorder_state;
#[path = "windows/selection_move.rs"]
mod selection_move;

#[test]
fn windows_moves_selected_roots_to_group() {
    selection_move::moves();
}

#[test]
fn windows_reviews_selected_move() {
    selection_move::reviews();
}
#[path = "../../mobile-edit-host/tests/fixture/native_table.rs"]
mod native_table;
#[path = "windows/navigation.rs"]
mod navigation;

#[test]
fn windows_opens_and_folds_main_table() {
    main_table::folds();
}

#[test]
fn windows_recovers_main_table_fold() {
    main_table::recovers();
}

#[test]
fn windows_creates_and_renames_grouped_root() {
    grouped_entry::creates_and_renames();
}

#[test]
fn windows_recovers_grouped_item_entry() {
    grouped_entry::recovers();
}
#[path = "../../mobile-edit-host/tests/fixture/peer.rs"]
mod peer;
#[path = "../../mobile-edit-host/tests/fixture/selection.rs"]
mod selection;
#[path = "../../mobile-edit-host/tests/fixture/stored_session.rs"]
mod stored_session;

#[test]
fn configured_logout_failure_then_retry() {
    logout::configured();
}

#[test]
fn interactive_viewer_logs_out() {
    logout::interactive();
}

#[test]
fn opens_selected_board_by_id() {
    navigation::run(false);
}

#[test]
fn revoked_membership_blocks_board_selection() {
    navigation::run(true);
}

#[test]
#[cfg(feature = "attended-clipboard")]
#[ignore = "requires an isolated clipboard or an attended handoff"]
fn copies_selected_board_link_and_opens_window() {
    board_link::open_window();
}

#[test]
#[cfg(feature = "attended-clipboard")]
#[ignore = "requires an isolated clipboard or an attended handoff"]
fn copied_board_link_checks_recipient_access() {
    board_link::recipient();
}

// Bound startup, rendering, and shutdown even if the native event loop stalls.
struct SmokeProcess(Child);
impl Drop for SmokeProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn connected_command(port: u16, board: Uuid, mode: &str) -> Command {
    let mut cmd = http_window_command();
    cmd.args(["connect", &port.to_string(), &board.to_string(), mode]);
    cmd
}

fn http_window_command() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_boardcraft-desktop"));
    cmd.env_remove("RUSTDAY_TEST_DATABASE_URL")
        .env_remove("DATABASE_URL")
        .env_remove("BOARDCRAFT_LOCAL_ACTOR")
        .env_remove("BOARDCRAFT_RECOVERY_DIR")
        .env_remove("BOARDCRAFT_SESSION_TOKEN")
        .env_remove("BOARDCRAFT_SESSION_ORIGIN")
        .stdout(Stdio::piped());
    cmd
}

struct Window(SmokeProcess, mpsc::Receiver<String>);
impl Window {
    fn launch(mut cmd: Command) -> Self {
        let mut child = SmokeProcess(cmd.stdin(Stdio::piped()).spawn().unwrap());
        let rx = lines(&mut child);
        Self(child, rx)
    }
    fn expect(&self, name: &str) {
        let line = self
            .1
            .recv_timeout(Duration::from_secs(45))
            .unwrap_or_else(|error| panic!("waiting for {name}: {error}"));
        assert!(line == format!("check {name}"), "expected {name}");
    }
    fn reply(&mut self, reply: &str) {
        writeln!(self.0.0.stdin.as_mut().unwrap(), "{reply}").unwrap();
    }
    fn finish(mut self) {
        self.reply("ok");
        wait_for_close(&mut self.0);
    }
}

fn connected_rows() -> [boardcraft_mobile_edit_client::SavedItem; 2] {
    use boardcraft_mobile_edit_client::SavedItem;
    use fixture::{FIRST, SECOND};
    [(FIRST, "Review sketch"), (SECOND, "Ink")].map(|(id, title)| SavedItem {
        id,
        title: title.into(),
        revision: 1,
    })
}

#[test]
fn windows_login_reject_retry() {
    let f = fixture::Fixture::with_titles(["Review sketch", "Ink"]);
    let secret = Uuid::new_v4().simple().to_string();
    let member = f.seed_account("boardmember", &secret, boardcraft_account::Role::Member);
    f.set_creator(member);
    let rows = connected_rows();

    let mut child = SmokeProcess(
        connected_command(f.host.port, f.board, "--smoke-login")
            .stdin(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let rx = lines(&mut child);
    let timeout = Duration::from_secs(45);
    let rejected = rx
        .recv_timeout(timeout)
        .expect("expected rejection checkpoint");
    assert!(rejected == "check rejected", "unexpected checkpoint");
    let sessions = || {
        f.connect()
            .query("SELECT account_id FROM account_sessions", &[])
            .unwrap()
            .into_iter()
            .map(|r| r.get::<_, Uuid>(0))
            .collect::<Vec<_>>()
    };
    assert!(sessions().is_empty());
    f.check_store(&rows);
    writeln!(child.0.stdin.take().unwrap(), "{secret}").unwrap();
    let opened = rx.recv_timeout(timeout).expect("expected displayed board");
    assert!(
        opened == format!("opened {}", f.board),
        "unexpected displayed board"
    );
    assert_eq!(sessions(), [member]);
    f.check_store(&rows);
    wait_for_close(&mut child);
}

fn wait_for_close(child: &mut SmokeProcess) {
    let until = Instant::now() + Duration::from_secs(45);
    loop {
        if let Some(exit) = child.0.try_wait().unwrap() {
            assert!(exit.success());
            break;
        }
        assert!(Instant::now() < until, "window did not close");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn windows_env_session_is_origin_scoped() {
    use std::{io::ErrorKind, net::TcpListener};
    use stored_session::StoredSession;

    let f = fixture::Fixture::with_titles(["Review sketch", "Ink"]);
    let password = Uuid::new_v4().simple().to_string();
    let member = f.seed_account("boardmember", &password, boardcraft_account::Role::Member);
    let token = boardcraft_account::sign_in(&f.schema.config, "boardmember", &password).unwrap();
    let original = StoredSession::only(&f, member);
    let port = f.host.port;
    let origin = format!("http://127.0.0.1:{port}");
    let launch = |port, origin: Option<&str>, token: Option<&str>, expected: &str| {
        let mut cmd = connected_command(port, f.board, "--smoke-session");
        if let Some(origin) = origin {
            cmd.env("BOARDCRAFT_SESSION_ORIGIN", origin);
        }
        if let Some(token) = token {
            cmd.env("BOARDCRAFT_SESSION_TOKEN", token);
        }
        let mut child = SmokeProcess(cmd.stdin(Stdio::null()).spawn().unwrap());
        let screen = lines(&mut child)
            .recv_timeout(Duration::from_secs(45))
            .expect("expected session screen");
        assert!(screen == expected, "unexpected session screen");
        wait_for_close(&mut child);
    };

    // A queued connection is observable even after the child exits. Never read its contents.
    let other = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    other.set_nonblocking(true).unwrap();
    let other_port = other.local_addr().unwrap().port();
    let other_origin = format!("http://127.0.0.1:{other_port}");
    let bearer = Some(token.encoded());
    launch(other_port, None, None, "identifier");
    launch(other_port, Some(&origin), bearer, "ignored");
    launch(other_port, Some(&other_origin), None, "ignored");
    launch(other_port, None, bearer, "ignored");
    launch(
        other_port,
        Some(&other_origin),
        Some("malformed"),
        "ignored",
    );
    assert!(matches!(other.accept(), Err(e) if e.kind() == ErrorKind::WouldBlock));

    launch(port, Some(&origin), bearer, "denied");
    f.set_creator(member);
    launch(port, Some(&origin), bearer, &format!("opened {}", f.board));
    assert!(StoredSession::only(&f, member) == original);
    f.check_store(&connected_rows());
    original.revoke(&f);
    launch(port, Some(&origin), bearer, "expired");
    stored_session::assert_no_session(&f);
}

fn lines(child: &mut SmokeProcess) -> mpsc::Receiver<String> {
    let out = child.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

struct NameFixture {
    f: fixture::Fixture,
    member: Uuid,
    token: boardcraft_account::Token,
    original: stored_session::StoredSession,
}

impl NameFixture {
    fn new() -> Self {
        let f = fixture::Fixture::start();
        let password = Uuid::new_v4().simple().to_string();
        let account = |name| f.seed_account(name, &password, boardcraft_account::Role::Member);
        f.set_creator(account("owner"));
        let member = account("boardmember");
        f.connect()
            .execute(
                "INSERT INTO board_subscribers(board_id,account_id) VALUES ($1,$2)",
                &[&f.board, &member],
            )
            .unwrap();
        let token =
            boardcraft_account::sign_in(&f.schema.config, "boardmember", &password).unwrap();
        let original = stored_session::StoredSession::only(&f, member);
        Self {
            f,
            member,
            token,
            original,
        }
    }

    fn exercise(&self, mode: &str, checks: &[&str]) {
        let f = &self.f;
        let mut child = SmokeProcess(
            connected_command(f.host.port, f.board, mode)
                .env(
                    "BOARDCRAFT_SESSION_ORIGIN",
                    format!("http://127.0.0.1:{}", f.host.port),
                )
                .env("BOARDCRAFT_SESSION_TOKEN", self.token.encoded())
                .stdin(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let rx = lines(&mut child);
        let mut db = f.connect();
        for &check in checks {
            let line = rx
                .recv_timeout(Duration::from_secs(45))
                .expect("expected name checkpoint");
            assert!(line == format!("check {check}"), "expected {check}");
            let (second, revision) = if matches!(check, "saved" | "reopened") {
                ("Coral", 2)
            } else {
                ("Review", 1)
            };
            f.check_store(
                &[
                    (fixture::FIRST, "Review", 1),
                    (fixture::SECOND, second, revision),
                ]
                .map(|(id, title, revision)| {
                    boardcraft_mobile_edit_client::SavedItem {
                        id,
                        title: title.into(),
                        revision,
                    }
                }),
            );
            assert!(stored_session::StoredSession::only(f, self.member) == self.original);
            match check {
                "draft" if mode == "--name-demote" => {
                    assert_eq!(
                        db.execute(
                            "UPDATE accounts SET role='viewer' WHERE id=$1",
                            &[&self.member]
                        )
                        .unwrap(),
                        1
                    );
                }
                "draft" => {
                    db.batch_execute("BEGIN").unwrap();
                    db.query_one(
                        "SELECT id FROM board_items WHERE board_id=$1 AND id=$2 FOR UPDATE",
                        &[&f.board, &fixture::SECOND],
                    )
                    .unwrap();
                }
                "pending" => db.batch_execute("COMMIT").unwrap(),
                _ => (),
            }
            writeln!(child.0.stdin.as_mut().unwrap(), "ok").unwrap();
        }
        wait_for_close(&mut child);
    }
}

#[test]
fn connected_member_renames_then_reopens() {
    let journey = NameFixture::new();
    journey.exercise(
        "--name-member",
        &[
            "opened",
            "cancelled",
            "unchanged",
            "draft",
            "pending",
            "saved",
        ],
    );
    journey.exercise("--name-reopen", &["reopened"]);
}

#[test]
fn connected_viewer_cannot_rename() {
    let journey = NameFixture::new();
    journey.exercise("--name-demote", &["opened", "draft", "denied"]);
    journey.exercise("--name-viewer", &["read_only"]);
}

fn exercise(fixture: &Fixture, mode: &str, checks: &[&str], check: impl FnMut(&str)) {
    exercise_as(
        fixture,
        fixture.recovery.path(),
        fixture.actor,
        mode,
        checks,
        check,
    );
}

fn exercise_as(
    fixture: &Fixture,
    root: &std::path::Path,
    actor: Uuid,
    mode: &str,
    checks: &[&str],
    mut check: impl FnMut(&str),
) {
    let board = fixture.board.id();
    let mut child = SmokeProcess(
        Command::new(env!("CARGO_BIN_EXE_boardcraft-desktop"))
            .arg(board.as_uuid().to_string())
            .arg(mode)
            .env("RUSTDAY_TEST_DATABASE_URL", &fixture.url)
            .env("BOARDCRAFT_LOCAL_ACTOR", actor.to_string())
            .env("BOARDCRAFT_RECOVERY_DIR", root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let rx = lines(&mut child);
    let until = Instant::now() + Duration::from_secs(45);
    let mut presented = 0;
    let mut checkpoints = checks.iter();
    loop {
        match rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
            Ok(line) => {
                println!("{mode}: {line}");
                if line.starts_with("row ") {
                    let item = &fixture.board.items()[presented];
                    let label = match (presented, mode) {
                        (1, "--smoke-reopen") => "Review artwork",
                        (1, "--smoke-committed") => "Coral",
                        (1, "--draft-coral") => "Indigo",
                        (1, "--restore-amber" | "--restore-with-failure") => " Amber ",
                        (1, "--restore-empty") => "",
                        (1, "--restore-coral") => " Coral ",
                        _ => "Review",
                    };
                    assert_eq!(
                        line,
                        format!("row {} {} {label}", board.as_uuid(), item.id.as_uuid())
                    );
                    presented += 1;
                } else if let Some(name) = line.strip_prefix("check ") {
                    assert_eq!(presented, 2);
                    assert_eq!(checkpoints.next().copied(), Some(name));
                    check(name);
                    writeln!(child.0.stdin.as_mut().unwrap(), "ok").unwrap();
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(_) => panic!("{mode}: timed out at {checkpoints:?}"),
        }
    }
    loop {
        if let Some(exit) = child.0.try_wait().unwrap() {
            assert!(exit.success(), "{mode}: exit={exit}");
            assert_eq!(presented, 2);
            assert!(checkpoints.next().is_none());
            break;
        }
        assert!(Instant::now() < until, "window did not close");
        thread::sleep(Duration::from_millis(10));
    }
}

struct Fixture {
    recovery: tempfile::TempDir,
    actor: Uuid,
    schema: String,
    url: String,
    cfg: Config,
    board: Board,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Ok(mut db) = self.cfg.connect(NoTls) {
            db.batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema))
                .expect("drop smoke schema");
        }
    }
}

fn fixture() -> Fixture {
    let mut db = database().unwrap().connect(NoTls).unwrap();
    let schema = format!("windows_test_{}", Uuid::new_v4().simple());
    let base = std::env::var("RUSTDAY_TEST_DATABASE_URL").unwrap();
    let separator = if base.contains('?') { '&' } else { '?' };
    let url =
        format!("{base}{separator}options=-csearch_path%3D{schema}%20-cstatement_timeout%3D15000");
    let cfg: Config = url.parse().expect("scoped database URL");
    db.batch_execute(&format!(
        "CREATE SCHEMA {schema}; SET search_path TO {schema}"
    ))
    .unwrap();
    store::migrate(&mut db).unwrap();
    let id = BoardId::new(Uuid::new_v4()).unwrap();
    let second = ItemId::new(Uuid::from_u128(17)).unwrap();
    let mut expected = Board::new(id);
    expected
        .add_item(ItemId::new(Uuid::from_u128(42)).unwrap(), "Review")
        .unwrap();
    expected.add_item(second, "Review").unwrap();
    store::create_board(&mut db, &expected).unwrap();
    Fixture {
        recovery: tempfile::tempdir().unwrap(),
        actor: Uuid::new_v4(),
        schema,
        url,
        cfg,
        board: expected,
    }
}

fn readback(fixture: &Fixture, title: &str, revision: i64) {
    let board = &fixture.board;
    let mut fresh = fixture.cfg.connect(NoTls).unwrap();
    let rows = store::load_items(&mut fresh, board.id()).unwrap().unwrap();
    let actual: Vec<_> = rows
        .iter()
        .map(|row| (row.id, row.title.as_str(), row.revision))
        .collect();
    assert_eq!(
        actual,
        [
            (board.items()[0].id, "Review", 1),
            (board.items()[1].id, title, revision)
        ]
    );
}

#[test]
fn unsubmitted_name_reopens_with_entry_baseline() {
    let f = fixture();
    let original = |_: &str| readback(&f, "Review", 1);
    exercise(&f, "--draft-untouched", &["untouched"], original);
    exercise(&f, "--smoke-clean", &["clean"], original);
    exercise(&f, "--draft-amber", &["close_draft"], original);
    exercise_as(
        &f,
        f.recovery.path(),
        Uuid::new_v4(),
        "--smoke-clean",
        &["clean"],
        original,
    );
    let other = fixture();
    exercise_as(
        &other,
        f.recovery.path(),
        f.actor,
        "--smoke-clean",
        &["clean"],
        |_| readback(&other, "Review", 1),
    );
    exercise(
        &f,
        "--restore-amber",
        &["draft_restored", "finished"],
        original,
    );
    exercise(&f, "--smoke-clean", &["clean"], original);

    exercise(&f, "--smoke-fail-blank", &["rejected"], original);
    exercise(&f, "--draft-with-failure", &["close_draft"], original);
    exercise(
        &f,
        "--restore-with-failure",
        &["both_restored", "failure_kept"],
        original,
    );
    exercise(
        &f,
        "--smoke-restore-dismiss",
        &["restored", "dismissed"],
        original,
    );

    let independent_edit = |before, after, revision| {
        readback(&f, before, revision - 1);
        let mut db = f.cfg.connect(NoTls).unwrap();
        store::rename_item_if_current(&mut db, f.board.id(), f.board.items()[1].id, before, after)
            .unwrap();
        readback(&f, after, revision);
    };
    exercise(&f, "--draft-empty", &["close_draft"], original);
    independent_edit("Review", "Indigo", 2);
    exercise(
        &f,
        "--restore-empty",
        &["draft_restored", "finished"],
        |_| readback(&f, "Indigo", 2),
    );
    exercise(&f, "--draft-coral", &["close_draft"], |_| {
        readback(&f, "Indigo", 2)
    });
    independent_edit("Indigo", "Silver", 3);
    exercise(
        &f,
        "--restore-coral",
        &["draft_restored", "finished"],
        |stage| match stage {
            "draft_restored" => readback(&f, "Silver", 3),
            "finished" => readback(&f, "Coral", 4),
            _ => unreachable!(),
        },
    );
    exercise(&f, "--smoke-committed", &["clean"], |_| {
        readback(&f, "Coral", 4)
    });
}

#[test]
fn failed_name_reopens_only_for_actor_and_board() {
    let f = fixture();
    let original = |_: &str| readback(&f, "Review", 1);
    exercise(&f, "--smoke-fail-blank", &["rejected"], original);
    exercise(
        &f,
        "--smoke-restore-dismiss",
        &["restored", "dismissed"],
        original,
    );
    exercise(&f, "--smoke-clean", &["clean"], original);

    // Exercise an actual edit in the degraded window: no implicit repair on
    // load, and recording its rejection must not replace the unreadable bytes.
    let file = f
        .recovery
        .path()
        .join(f.actor.to_string())
        .join(f.board.id().as_uuid().to_string())
        .join("names.json");
    let corrupt = b"unfinished synthetic name recovery";
    std::fs::write(&file, corrupt).unwrap();
    exercise(
        &f,
        "--smoke-unreadable",
        &["unreadable", "rejected"],
        |name| {
            original(name);
            assert_eq!(std::fs::read(&file).unwrap(), corrupt);
        },
    );
    std::fs::remove_file(file).unwrap();

    let mut db = f.cfg.connect(NoTls).unwrap();
    db.batch_execute(
        "ALTER TABLE board_items ADD CONSTRAINT temporary_fault CHECK (title <> 'Coral')",
    )
    .unwrap();
    exercise(&f, "--smoke-fail-coral", &["write_failed"], original);
    exercise_as(
        &f,
        f.recovery.path(),
        Uuid::new_v4(),
        "--smoke-clean",
        &["clean"],
        original,
    );
    let other = fixture(); // separate schema permits the same item IDs on a different board
    exercise_as(
        &other,
        f.recovery.path(),
        f.actor,
        "--smoke-clean",
        &["clean"],
        |_| readback(&other, "Review", 1),
    );

    db.batch_execute("ALTER TABLE board_items DROP CONSTRAINT temporary_fault")
        .unwrap();
    let mut lock = db.transaction().unwrap();
    lock.query_one(
        "SELECT id FROM board_items WHERE board_id=$1 AND id=$2 FOR UPDATE",
        &[&f.board.id().as_uuid(), &f.board.items()[1].id.as_uuid()],
    )
    .unwrap();
    let mut lock = Some(lock);
    exercise(
        &f,
        "--smoke-restore-retry",
        &["restored", "retry_pending", "retry_saved"],
        |name| {
            if name == "retry_saved" {
                readback(&f, "Coral", 2);
            } else {
                original(name);
            }
            if name == "retry_pending" {
                // A pending frame alone does not prove the worker reached SQL.
                let until = Instant::now() + Duration::from_secs(5);
                let tx = lock.as_mut().unwrap();
                let has_waiter = "SELECT EXISTS (SELECT FROM pg_locks WHERE NOT granted
                                  AND pg_backend_pid() = ANY(pg_blocking_pids(pid)))";
                while !tx.query_one(has_waiter, &[]).unwrap().get::<_, bool>(0) {
                    assert!(
                        Instant::now() < until,
                        "retry did not reach the fixture lock"
                    );
                    thread::sleep(Duration::from_millis(10));
                }
                exercise(&f, "--smoke-clean", &["clean"], original);
                lock.take().unwrap().commit().unwrap();
            }
        },
    );
    exercise(&f, "--smoke-committed", &["clean"], |_| {
        readback(&f, "Coral", 2)
    });
}

#[test]
fn rejected_edit_then_save_close_and_reopen() {
    let expected = fixture();
    let id = expected.board.id();
    let second = expected.board.items()[1].id;
    let mut db = expected.cfg.connect(NoTls).unwrap();
    let mut tx = db.transaction().unwrap();
    tx.query_one(
        "SELECT id FROM board_items WHERE board_id=$1 AND id=$2 FOR UPDATE",
        &[&id.as_uuid(), &second.as_uuid()],
    )
    .unwrap();
    let mut lock = Some(tx);
    exercise(
        &expected,
        "--smoke-edit",
        &["rejected", "pending", "saved"],
        |name| {
            if name == "saved" {
                readback(&expected, "Review artwork", 2);
            } else {
                readback(&expected, "Review", 1);
            }
            if name == "pending" {
                lock.take().unwrap().commit().unwrap();
            }
        },
    );
    assert!(lock.is_none());
    exercise(&expected, "--smoke-reopen", &["reopened"], |_| {
        readback(&expected, "Review artwork", 2)
    });
}

#[test]
fn display_edit_cancel_blur_and_enter() {
    let expected = fixture();
    exercise(
        &expected,
        "--smoke-cell",
        &["cancelled", "unchanged", "blurred", "normalized", "entered"],
        |name| match name {
            "cancelled" | "unchanged" => readback(&expected, "Review", 1),
            "blurred" | "normalized" => readback(&expected, "Coral", 2),
            "entered" => readback(&expected, "Review artwork", 3),
            _ => unreachable!(),
        },
    );
}

#[test]
fn parent_failure_retry_dismiss_and_commit() {
    let f = fixture();
    let mut db = f.cfg.connect(NoTls).unwrap();
    let second = f.board.items()[1].id.as_uuid();
    exercise(
        &f,
        "--smoke-recovery",
        &[
            "rejected",
            "cancel_kept",
            "unchanged_kept",
            "retried_blank",
            "dismissed",
            "write_failed",
            "retry_pending",
            "retry_saved",
        ],
        |name| {
            if name == "retry_saved" {
                readback(&f, "Coral", 2);
            } else {
                readback(&f, "Review", 1);
            }
            match name {
                "dismissed" => db.batch_execute(
                    "ALTER TABLE board_items ADD CONSTRAINT temporary_fault CHECK (title <> 'Coral')"
                ).unwrap(),
                "write_failed" => {
                    db.batch_execute("ALTER TABLE board_items DROP CONSTRAINT temporary_fault").unwrap();
                    db.batch_execute("BEGIN").unwrap();
                    db.query_one("SELECT id FROM board_items WHERE id=$1 FOR UPDATE", &[&second]).unwrap();
                }
                "retry_pending" => db.batch_execute("COMMIT").unwrap(),
                _ => (),
            }
        },
    );
}
