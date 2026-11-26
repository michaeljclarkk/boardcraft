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

// Bound startup, rendering, and shutdown even if the native event loop stalls.
struct SmokeProcess(Child);
impl Drop for SmokeProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
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
    let out = child.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let until = Instant::now() + Duration::from_secs(45);
    let mut presented = 0;
    let mut checkpoints = checks.iter();
    loop {
        match rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
            Ok(line) => {
                println!("{mode}: {line}");
                if line.starts_with("row ") {
                    let item = &fixture.board.items()[presented];
                    let label = if mode == "--smoke-reopen" && presented == 1 {
                        "Review artwork"
                    } else if mode == "--smoke-committed" && presented == 1 {
                        "Coral"
                    } else {
                        "Review"
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
