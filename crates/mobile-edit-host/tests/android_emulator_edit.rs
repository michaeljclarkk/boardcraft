mod emulator;
mod fixture;

use boardcraft_mobile_edit_client::{EditError, SavedItem};
use emulator::{Adb, Probe};
use fixture::{FIRST, Fixture, SECOND};

fn open(f: &Fixture, probe: &Probe) -> Vec<SavedItem> {
    let rows = probe.read(f.board);
    assert_eq!(rows.len(), 2);
    for (row, id) in rows.iter().zip([FIRST, SECOND]) {
        assert_eq!(row.id, id);
        assert_eq!(row.title, "Review");
        assert_eq!(row.revision, 1);
    }
    rows
}

fn save_and_reopen(f: &Fixture, probe: &Probe, mut rows: Vec<SavedItem>, draft: &str, title: &str) {
    let saved = probe.rename(f.board, &rows[1], draft).unwrap();
    assert_eq!(saved.id, SECOND);
    assert_eq!(saved.title, title);
    assert!(saved.revision > rows[1].revision);
    rows[1] = saved;
    assert_eq!(probe.read(f.board), rows);
    f.check_store(&rows);
}

#[test]
fn emulator_rename_then_reopen() {
    let adb = Adb::dedicated();
    let f = Fixture::start();
    let probe = Probe::install(adb, f.host.port);
    let rows = open(&f, &probe);
    save_and_reopen(&f, &probe, rows, " Review artwork ", "Review artwork");
}

#[test]
fn emulator_reject_then_correct() {
    let adb = Adb::dedicated();
    let f = Fixture::start();
    let probe = Probe::install(adb, f.host.port);
    let rows = open(&f, &probe);
    assert_eq!(
        probe.rename(f.board, &rows[1], "   "),
        Err(EditError::InvalidDraft)
    );
    let reopened = probe.read(f.board);
    assert_eq!(reopened, rows);
    f.check_store(&rows);
    save_and_reopen(&f, &probe, reopened, " Corrected ", "Corrected");
}
