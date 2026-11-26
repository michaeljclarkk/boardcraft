//! Fixed journeys in a normally presented Kit window. Only production controls handle input.
use super::*;
use gpui_kit::test::{ElementSnapshot, TestEventExt, TestWindowExt};

#[derive(Debug, Copy, Clone)]
enum Step {
    Open(bool),
    Focus(bool),
    Editor(bool),
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
    Blank,
    Rejected,
    Corrected,
    Pending,
    Saved,
    Reopen,
    Delivery,
}

pub(super) fn install(window: AnyWindowHandle, board: Entity<Workbench>, cx: &mut App) {
    let step = match std::env::args().nth(2).as_deref() {
        Some("--smoke-edit") => Step::Open(false),
        Some("--smoke-cell") => Step::Open(true),
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
            if matches!(step, Step::Open(_) | Step::Reopen | Step::Delivery) {
                assert!(editors.is_empty());
                assert_eq!(r.edit.phase(), &Phase::Clean);
                let label = control.label().expect("name display label");
                assert_eq!(label, r.edit.accepted());
                println!("row {} {} {label}", id.as_uuid(), item.as_uuid());
            }
        }
        if matches!(step, Step::Delivery) {
            w.remove_window();
            return;
        }
        assert_eq!(first.label(), Some("Review"));
        let next = match step {
            Step::Open(cell) => {
                assert_eq!(second.label(), Some("Review"));
                click(second, w, cx);
                Step::Focus(cell)
            }
            Step::Focus(cell) => {
                assert_eq!(second.label(), Some("Review"));
                assert_eq!(second.focused(), Some(true));
                w.double_click(row.title_id(), cx);
                Step::Editor(cell)
            }
            Step::Editor(cell) => {
                editor(second, "Review");
                replace(if cell { "Discard me" } else { "   " }, w, cx);
                if cell { Step::CancelDraft } else { Step::Blank }
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
            Step::Blank => {
                editor(second, "   ");
                w.press("enter", cx);
                Step::Rejected
            }
            Step::Rejected if message(w, "Not saved: enter a title.") => {
                editor(second, "   ");
                assert_eq!(row.edit.draft(), "   ");
                assert_eq!(w.find("retry").label(), Some("Retry"));
                checkpoint("rejected");
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
                assert_eq!(second.label(), Some("Review"));
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
