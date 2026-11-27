//! Fixed journeys in a normally presented Kit window. Only production controls handle input.
use super::*;
use gpui_kit::test::{ElementSnapshot, TestEventExt, TestWindowExt};
pub(super) mod archive;
pub(super) mod board_link;
pub(super) mod group_fields;
pub(super) mod group_order;
pub(super) mod grouped_entry;
pub(super) mod login;
pub(super) mod logout;
pub(super) mod main_table;
pub(super) mod name;
pub(super) mod navigation;
pub(super) mod selection_move;

#[derive(Debug, Copy, Clone)]
enum Journey {
    Cell,
    ReopenEdit,
    Recovery,
    FailBlank,
    Unreadable,
    FailCoral,
}

#[derive(Debug, Copy, Clone)]
enum Step {
    DraftStart(&'static str, Option<&'static str>), // accepted display, replacement (None = untouched)
    DraftInput(Option<&'static str>),
    DraftClose(&'static str),
    DraftRestore(&'static str),
    DraftFinishEntry,
    DraftExit(&'static str, usize),
    OpenAlongsideFailure,
    RestoreAlongsideFailure,
    EscapedAlongsideFailure,
    Open(Journey),
    Focus(Journey),
    Editor(Journey),
    CancelDraft,
    Cancelled,
    UnchangedEditor,
    Skipped,
    BlurEditor,
    BlurDraft,
    Blurred,
    PaddedEditor,
    PaddedDraft,
    Normalized,
    EnterEditor,
    EnterDraft,
    Entered,
    Blank(Journey),
    Rejected(Journey),
    CorrectionEditor,
    NewEditor(bool),
    FailureKept(bool),
    RetriedBlank,
    Dismissed,
    FaultEditor,
    FaultDraft,
    WriteFailed(bool),  // close on failure, or retry in this process
    RetryPending(bool), // restored request
    RetrySaved(bool),
    Restore(bool), // retry the database failure, or dismiss the blank request
    RestoredDismissed,
    CleanReopen,
    Corrected,
    Pending,
    Saved,
    Reopen,
    Delivery,
}

pub(super) fn install(window: AnyWindowHandle, board: Entity<Workbench>, cx: &mut App) {
    let step = match std::env::args().nth(2).as_deref() {
        Some("--draft-with-failure") => Step::OpenAlongsideFailure,
        Some("--restore-with-failure") => Step::RestoreAlongsideFailure,
        Some("--draft-untouched") => Step::DraftStart("Review", None),
        Some("--draft-amber") => Step::DraftStart("Review", Some(" Amber ")),
        Some("--draft-empty") => Step::DraftStart("Review", Some("")),
        Some("--draft-coral") => Step::DraftStart("Indigo", Some(" Coral ")),
        Some("--restore-amber") => Step::DraftRestore(" Amber "),
        Some("--restore-empty") => Step::DraftRestore(""),
        Some("--restore-coral") => Step::DraftRestore(" Coral "),
        Some("--smoke-edit") => Step::Open(Journey::ReopenEdit),
        Some("--smoke-cell") => Step::Open(Journey::Cell),
        Some("--smoke-recovery") => Step::Open(Journey::Recovery),
        Some("--smoke-fail-blank") => Step::Open(Journey::FailBlank),
        Some("--smoke-unreadable") => Step::Open(Journey::Unreadable),
        Some("--smoke-fail-coral") => Step::Open(Journey::FailCoral),
        Some("--smoke-restore-retry") => Step::Restore(true),
        Some("--smoke-restore-dismiss") => Step::Restore(false),
        Some("--smoke-clean" | "--smoke-committed") => Step::CleanReopen,
        Some("--smoke-reopen") => Step::Reopen,
        Some("--smoke-delivery") => Step::Delivery,
        _ => return,
    };
    cx.update_window(window, |_, w, _| schedule(step, board, w))
        .unwrap();
}

fn click(target: &ElementSnapshot, w: &mut Window, cx: &mut App) {
    assert!(target.visible());
    let p = target.bounds().center();
    w.pointer_move(p, None, cx);
    w.pointer_down(p, MouseButton::Left, cx);
    w.pointer_up(p, MouseButton::Left, cx);
}

fn message(w: &Window, expected: &str) -> bool {
    base::test_support::texts(w)
        .iter()
        .any(|t| t.visible() && t.text() == expected)
}

// The parent acknowledges only after its independent read. No later UI event
// can race that assertion. Its timeout also bounds a stalled window.
fn checkpoint(name: &str) {
    println!("check {name}");
    let mut reply = String::new();
    std::io::stdin().read_line(&mut reply).unwrap();
    assert_eq!(reply.trim_end(), "ok");
}

fn replace(text: &str, w: &mut Window, cx: &mut App) {
    w.press("ctrl-a", cx);
    w.input(text, cx);
}

fn editor(control: &ElementSnapshot, value: &str) {
    assert_eq!(control.value(), Some(value));
    assert_eq!(control.focused(), Some(true));
}

fn schedule(step: Step, board: Entity<Workbench>, w: &Window) {
    w.on_next_frame(move |w, cx| {
        let view = board.read(cx);
        assert!(!view.failed, "{}", view.message);
        if view.rows.is_empty() {
            return schedule(step, board, w);
        }
        let [first_row, second_row] = &view.rows[..] else {
            panic!("expected two rows")
        };
        let first_row = first_row.read(cx);
        let row = second_row.read(cx);
        let first_hits = w.find_all(first_row.title_id());
        if first_hits.is_empty() {
            return schedule(step, board, w);
        }
        assert_eq!(first_hits.len(), 1);
        let first = &first_hits[0];
        assert!(w.find_all(first_row.input_id()).is_empty());
        let displays = w.find_all(row.title_id());
        let editors = w.find_all(row.input_id());
        let second = match (&displays[..], &editors[..]) {
            ([display], []) => display,
            ([], [editor]) => editor,
            _ => panic!("expected one mounted second name control at {step:?}"),
        };
        assert!(
            first.visible() && second.visible() && first.bounds().bottom() <= second.bounds().top()
        );
        assert_eq!(first_row.edit.phase(), &Phase::Clean);
        for (r, control) in [(first_row, first), (row, second)] {
            let (id, item) = r.edit.identity();
            assert_eq!(id, view.board);
            if matches!(
                step,
                Step::Open(_)
                    | Step::DraftStart(..)
                    | Step::OpenAlongsideFailure
                    | Step::Reopen
                    | Step::Delivery
                    | Step::CleanReopen
                    | Step::Restore(_)
            ) {
                assert!(editors.is_empty());
                assert_eq!(r.submissions, 0);
                if !matches!(step, Step::Restore(_) | Step::OpenAlongsideFailure) {
                    assert_eq!(r.edit.phase(), &Phase::Clean);
                }
                let label = control.label().expect("name display label");
                assert_eq!(label, r.edit.accepted());
                println!("row {} {} {label}", id.as_uuid(), item.as_uuid());
            } else if matches!(step, Step::DraftRestore(_) | Step::RestoreAlongsideFailure) {
                if matches!(step, Step::DraftRestore(_)) { assert_eq!(r.edit.phase(), &Phase::Clean); }
                assert_eq!(r.submissions, 0);
                let text = control.value().or(control.label()).unwrap();
                println!("row {} {} {text}", id.as_uuid(), item.as_uuid());
            }
        }
        if matches!(step, Step::Delivery) {
            w.remove_window();
            return;
        }
        assert_eq!(first.label(), Some("Review"));
        let id = row.edit.identity().1.as_uuid();
        let retries = w.find_all(SharedString::from(format!("retry-{id}")));
        let dismissals = w.find_all(SharedString::from(format!("dismiss-{id}")));
        let actions = || {
            assert_eq!(retries.len(), 1);
            assert_eq!(dismissals.len(), 1);
            assert!(retries[0].visible() && dismissals[0].visible());
            assert!(retries[0].bounds().bottom() <= first.bounds().top());
            assert!(message(w, "Name change: Review"));
            assert_eq!(retries[0].label(), Some("Retry change"));
            assert_eq!(dismissals[0].label(), Some("Dismiss"));
        };
        let next = match step {
            Step::OpenAlongsideFailure => {
                actions();
                assert_eq!(row.edit.request(), Some("   "));
                w.double_click(row.title_id(), cx);
                Step::DraftInput(Some(" Amber "))
            }
            Step::RestoreAlongsideFailure => {
                actions();
                assert_eq!(row.edit.request(), Some("   "));
                editor(second, " Amber ");
                checkpoint("both_restored");
                w.press("escape", cx);
                Step::EscapedAlongsideFailure
            }
            Step::EscapedAlongsideFailure => {
                actions();
                assert_eq!(row.edit.request(), Some("   "));
                assert_eq!(second.label(), Some("Review"));
                assert_eq!(row.submissions, 0);
                checkpoint("failure_kept");
                w.remove_window(); return;
            }
            Step::DraftStart(accepted, raw) => {
                assert_eq!(second.label(), Some(accepted));
                assert!(retries.is_empty() && dismissals.is_empty());
                w.double_click(row.title_id(), cx);
                Step::DraftInput(raw)
            }
            Step::DraftInput(raw) => {
                editor(second, row.edit.accepted());
                assert_eq!(row.submissions, 0);
                match raw {
                    None => { checkpoint("untouched"); w.remove_window(); return; }
                    Some("") => { w.press("ctrl-a", cx); w.press("backspace", cx); }
                    Some(raw) => replace(raw, w, cx),
                }
                Step::DraftClose(raw.unwrap())
            }
            Step::DraftClose(raw) => {
                editor(second, raw);
                assert_eq!(row.submissions, 0);
                if let Phase::Failed(_) = row.edit.phase() {
                    actions();
                    assert_eq!(row.edit.request(), Some("   "));
                } else {
                    assert_eq!(row.edit.phase(), &Phase::Clean);
                }
                checkpoint("close_draft");
                w.remove_window(); return;
            }
            Step::DraftRestore(raw) => {
                editor(second, raw);
                assert!(retries.is_empty() && dismissals.is_empty());
                let (entry, revision, current, current_revision) = match raw {
                    " Amber " => ("Review", 1, "Review", 1),
                    "" => ("Review", 1, "Indigo", 2),
                    " Coral " => ("Indigo", 2, "Silver", 3),
                    _ => unreachable!(),
                };
                assert_eq!(row.baseline.as_ref(), Some(&(entry.into(), revision)));
                assert_eq!((row.edit.accepted(), row.edit.revision()), (current, current_revision));
                checkpoint("draft_restored");
                match raw {
                    " Amber " => { w.press("escape", cx); Step::DraftExit("Review", 0) }
                    "" => { replace(entry, w, cx); Step::DraftFinishEntry }
                    _ => { w.press("enter", cx); Step::DraftExit("Coral", 1) }
                }
            }
            Step::DraftFinishEntry => {
                editor(second, "Review");
                w.press("enter", cx);
                Step::DraftExit("Indigo", 0)
            }
            Step::DraftExit(title, submissions) if second.label() == Some(title) => {
                assert_eq!(row.submissions, submissions);
                assert_eq!(second.focused(), Some(true));
                assert!(retries.is_empty() && dismissals.is_empty());
                checkpoint("finished");
                w.remove_window(); return;
            }
            Step::Open(cell) => {
                assert_eq!(second.label(), Some("Review"));
                if matches!(cell, Journey::Unreadable) {
                    assert!(message(w, "Local name recovery is unreadable. Check the recovery directory before retrying."));
                    assert!(retries.is_empty() && dismissals.is_empty());
                    checkpoint("unreadable");
                }
                click(second, w, cx);
                Step::Focus(cell)
            }
            Step::Focus(cell) => {
                assert_eq!(second.label(), Some("Review"));
                assert_eq!(second.focused(), Some(true));
                w.double_click(row.title_id(), cx);
                Step::Editor(cell)
            }
            Step::Editor(journey) => {
                editor(second, "Review");
                if matches!(journey, Journey::FailCoral) {
                    replace(" Coral ", w, cx);
                    Step::FaultDraft
                } else if matches!(journey, Journey::Cell) {
                    replace("Discard me", w, cx);
                    Step::CancelDraft
                } else {
                    replace("   ", w, cx);
                    Step::Blank(journey)
                }
            }
            Step::CancelDraft => {
                editor(second, "Discard me");
                w.press("escape", cx);
                Step::Cancelled
            }
            Step::Cancelled | Step::Skipped => {
                assert_eq!(second.label(), Some("Review"));
                assert_eq!(second.focused(), Some(true));
                assert_eq!(row.submissions, 0);
                assert_eq!(row.edit.phase(), &Phase::Clean);
                let cancelled = matches!(step, Step::Cancelled);
                checkpoint(if cancelled { "cancelled" } else { "unchanged" });
                w.press("f2", cx);
                if cancelled {
                    Step::UnchangedEditor
                } else {
                    Step::BlurEditor
                }
            }
            Step::UnchangedEditor => {
                editor(second, "Review");
                w.press("enter", cx);
                Step::Skipped
            }
            Step::BlurEditor => {
                editor(second, "Review");
                replace(" Coral ", w, cx);
                Step::BlurDraft
            }
            Step::BlurDraft => {
                editor(second, " Coral ");
                click(first, w, cx);
                Step::Blurred
            }
            Step::Blurred if message(w, "Saved") => {
                assert_eq!(second.label(), Some("Coral"));
                assert_eq!(first.focused(), Some(true));
                assert_eq!(row.submissions, 1);
                checkpoint("blurred");
                w.double_click(row.title_id(), cx);
                Step::PaddedEditor
            }
            Step::PaddedEditor => {
                editor(second, "Coral");
                replace(" Coral ", w, cx);
                Step::PaddedDraft
            }
            Step::PaddedDraft => {
                editor(second, " Coral ");
                w.press("enter", cx);
                Step::Normalized
            }
            Step::Normalized if message(w, "Saved") => {
                assert_eq!(second.label(), Some("Coral"));
                assert_eq!(row.submissions, 2);
                checkpoint("normalized");
                w.press("f2", cx);
                Step::EnterEditor
            }
            Step::EnterEditor => {
                editor(second, "Coral");
                replace("Review artwork", w, cx);
                Step::EnterDraft
            }
            Step::EnterDraft => {
                editor(second, "Review artwork");
                w.press("enter", cx);
                Step::Entered
            }
            Step::Entered if message(w, "Saved") => {
                assert_eq!(second.label(), Some("Review artwork"));
                assert_eq!(second.focused(), Some(true));
                assert_eq!(row.submissions, 3);
                checkpoint("entered");
                w.remove_window();
                return;
            }
            Step::Blank(journey) => {
                editor(second, "   ");
                if matches!(journey, Journey::Recovery) {
                    click(first, w, cx);
                } else {
                    w.press("enter", cx);
                }
                Step::Rejected(journey)
            }
            Step::Rejected(journey) if message(w, "Not saved: enter a title.") => {
                actions();
                assert_eq!(second.label(), Some("Review"));
                assert_eq!(row.edit.request(), Some("   "));
                assert_eq!(row.submissions, 1);
                let recovery = matches!(journey, Journey::Recovery);
                assert_eq!(if recovery { first } else { second }.focused(), Some(true));
                checkpoint("rejected");
                if matches!(journey, Journey::FailBlank | Journey::Unreadable) {
                    w.remove_window();
                    return;
                }
                w.double_click(row.title_id(), cx);
                if recovery {
                    Step::NewEditor(true)
                } else {
                    Step::CorrectionEditor
                }
            }
            Step::NewEditor(cancel) => {
                actions();
                editor(second, "Review");
                if cancel {
                    replace("Other draft", w, cx);
                    w.press("escape", cx);
                } else {
                    click(first, w, cx);
                }
                Step::FailureKept(cancel)
            }
            Step::FailureKept(cancel) => {
                actions();
                assert_eq!(second.label(), Some("Review"));
                assert_eq!(if cancel { second } else { first }.focused(), Some(true));
                assert_eq!(row.submissions, 1);
                assert_eq!(row.edit.request(), Some("   "));
                checkpoint(if cancel {
                    "cancel_kept"
                } else {
                    "unchanged_kept"
                });
                if cancel {
                    w.press("f2", cx);
                    Step::NewEditor(false)
                } else {
                    click(&retries[0], w, cx);
                    Step::RetriedBlank
                }
            }
            Step::RetriedBlank
                if row.submissions == 2 && message(w, "Not saved: enter a title.") =>
            {
                actions();
                assert_eq!(second.label(), Some("Review"));
                assert_eq!(row.edit.request(), Some("   "));
                assert_eq!(first.focused(), Some(true));
                checkpoint("retried_blank");
                click(&dismissals[0], w, cx);
                Step::Dismissed
            }
            Step::Dismissed if retries.is_empty() => {
                assert!(retries.is_empty() && dismissals.is_empty());
                assert_eq!(row.edit.request(), None);
                assert_eq!(row.submissions, 2);
                assert_eq!(second.label(), Some("Review"));
                assert_eq!(first.focused(), Some(true));
                checkpoint("dismissed");
                w.double_click(row.title_id(), cx);
                Step::FaultEditor
            }
            Step::FaultEditor => {
                editor(second, "Review");
                replace(" Coral ", w, cx);
                Step::FaultDraft
            }
            Step::FaultDraft => {
                editor(second, " Coral ");
                let close = row.submissions == 0;
                click(first, w, cx);
                Step::WriteFailed(close)
            }
            Step::WriteFailed(close)
                if message(w, "Save failed. Check the connection and retry.") =>
            {
                actions();
                assert_eq!(second.label(), Some("Review"));
                assert_eq!(first.focused(), Some(true));
                assert_eq!(row.submissions, if close { 1 } else { 3 });
                assert_eq!(row.edit.request(), Some(" Coral "));
                checkpoint("write_failed");
                if close {
                    w.remove_window();
                    return;
                }
                click(&retries[0], w, cx);
                Step::RetryPending(false)
            }
            Step::Restore(retry) => {
                actions();
                let (raw, reason) = if retry {
                    (" Coral ", "Save failed. Check the connection and retry.")
                } else {
                    ("   ", "Not saved: enter a title.")
                };
                assert!(message(w, reason));
                assert_eq!(row.edit.request(), Some(raw));
                assert_eq!(second.label(), Some("Review"));
                checkpoint("restored");
                if retry {
                    click(&retries[0], w, cx);
                    Step::RetryPending(true)
                } else {
                    click(&dismissals[0], w, cx);
                    Step::RestoredDismissed
                }
            }
            Step::RestoredDismissed if retries.is_empty() => {
                assert!(dismissals.is_empty());
                assert_eq!(second.label(), Some("Review"));
                assert_eq!(row.edit.phase(), &Phase::Clean);
                assert_eq!(row.submissions, 0);
                checkpoint("dismissed");
                w.remove_window();
                return;
            }
            Step::CleanReopen => {
                assert!(retries.is_empty() && dismissals.is_empty());
                checkpoint("clean");
                w.remove_window();
                return;
            }
            Step::RetryPending(restored) if message(w, "Saving…") => {
                assert!(retries.is_empty() && dismissals.is_empty());
                assert_eq!(second.label(), Some(" Coral "));
                assert_eq!(row.edit.accepted(), "Review");
                assert_eq!(row.submissions, if restored { 1 } else { 4 });
                if !restored {
                    assert_eq!(first.focused(), Some(true));
                }
                checkpoint("retry_pending");
                Step::RetrySaved(restored)
            }
            Step::RetrySaved(restored) if message(w, "Saved") => {
                assert!(retries.is_empty() && dismissals.is_empty());
                assert_eq!(second.label(), Some("Coral"));
                assert_eq!(row.edit.request(), None);
                assert_eq!(row.submissions, if restored { 1 } else { 4 });
                if !restored {
                    assert_eq!(first.focused(), Some(true));
                }
                checkpoint("retry_saved");
                w.remove_window();
                return;
            }
            Step::CorrectionEditor => {
                editor(second, "Review");
                replace(" Review artwork ", w, cx);
                Step::Corrected
            }
            Step::Corrected => {
                editor(second, " Review artwork ");
                click(first, w, cx);
                Step::Pending
            }
            Step::Pending if message(w, "Saving…") => {
                assert_eq!(row.edit.phase(), &Phase::Pending);
                assert_eq!(second.label(), Some(" Review artwork "));
                assert_eq!(row.edit.accepted(), "Review");
                assert!(retries.is_empty() && dismissals.is_empty());
                checkpoint("pending");
                Step::Saved
            }
            Step::Saved if message(w, "Saved") => {
                assert_eq!(second.label(), Some("Review artwork"));
                assert_eq!(first.focused(), Some(true));
                assert_eq!(row.submissions, 2);
                checkpoint("saved");
                w.remove_window();
                return;
            }
            Step::Reopen => {
                assert_eq!(second.label(), Some("Review artwork"));
                checkpoint("reopened");
                w.remove_window();
                return;
            }
            _ => step,
        };
        schedule(next, board, w);
    });
}
