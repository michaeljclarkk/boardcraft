//! Real Windows processes; run in a graphical session with --features native-smoke.
#![cfg(windows)]

use boardcraft_board::{Board, BoardId, ItemId, store};
use boardcraft_desktop::database;
use postgres::NoTls;
use std::{
    io::{BufRead, BufReader},
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

fn exercise(board: BoardId, mode: &str, finished: &str, mut on_line: impl FnMut(&str)) {
    let mut child = SmokeProcess(
        Command::new(env!("CARGO_BIN_EXE_boardcraft-desktop"))
            .arg(board.as_uuid().to_string())
            .arg(mode)
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
    let mut completed = false;
    loop {
        match rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
            Ok(line) => {
                println!("{mode}: {line}");
                on_line(&line);
                completed |= line == finished;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(_) => panic!("{mode}: timed out waiting for {finished}"),
        }
    }
    loop {
        if let Some(exit) = child.0.try_wait().unwrap() {
            assert!(
                exit.success() && completed,
                "{mode}: exit={exit}, completed={completed}"
            );
            break;
        }
        assert!(Instant::now() < until, "window did not close");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn rejected_edit_then_save_close_and_reopen() {
    let cfg = database().unwrap();
    let mut db = cfg.connect(NoTls).unwrap();
    store::migrate(&mut db).unwrap();
    let id = BoardId::new(Uuid::new_v4()).unwrap();
    let second = ItemId::new(Uuid::new_v4()).unwrap();
    let mut expected = Board::new(id);
    expected
        .add_item(ItemId::new(Uuid::new_v4()).unwrap(), "Review")
        .unwrap();
    expected.add_item(second, "Review").unwrap();
    store::create_board(&mut db, &expected).unwrap();
    let mut tx = db.transaction().unwrap();
    tx.query_one(
        "SELECT id FROM board_items WHERE board_id=$1 AND id=$2 FOR UPDATE",
        &[&id.as_uuid(), &second.as_uuid()],
    )
    .unwrap();
    let mut lock = Some(tx);
    exercise(id, "--smoke-edit", "saved frame; closing", |line| {
        if line == "pending frame" {
            lock.take().unwrap().commit().unwrap();
        }
    });
    assert!(lock.is_none());
    expected.rename_item(second, "Review artwork").unwrap();
    let mut fresh = cfg.connect(NoTls).unwrap();
    assert_eq!(store::load_board(&mut fresh, id).unwrap(), Some(expected));
    exercise(
        id,
        "--smoke-reopen",
        "reopened ordered titles; closing",
        |_| {},
    );

    let mut cleanup = fresh.transaction().unwrap();
    cleanup
        .execute(
            "DELETE FROM board_items WHERE board_id=$1",
            &[&id.as_uuid()],
        )
        .unwrap();
    cleanup
        .execute("DELETE FROM boards WHERE id=$1", &[&id.as_uuid()])
        .unwrap();
    cleanup.commit().unwrap();
}
