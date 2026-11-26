//! Drive only visible controls. Count outgoing operations across a board re-read.
use super::*;
use crate::connected::Connected;

#[derive(Clone, Copy)]
enum Step {
    Start,
    Focused,
    EditCancel,
    Cancel,
    Cancelled,
    Unchanged,
    Skipped,
    EditName,
    Submit,
    Pending,
    Saved,
    Reopen,
    Denied,
    Viewer,
    AfterDoubleClick,
    AfterF2,
}

pub(crate) fn install(view: Entity<Connected>, mode: &str, w: &Window) {
    let step = match mode {
        "--name-member" | "--name-demote" => Step::Start,
        "--name-reopen" => Step::Reopen,
        "--name-viewer" => Step::Viewer,
        _ => unreachable!(),
    };
    schedule_name(view, step, mode == "--name-demote", w);
}

fn schedule_name(view: Entity<Connected>, step: Step, demote: bool, w: &Window) {
    use self::Step::*;
    w.on_next_frame(move |w, cx| {
        let id = |prefix, n| SharedString::from(format!("{prefix}-{}", Uuid::from_u128(n)));
        let first = w.find_all(id("title", 42));
        if first.is_empty() {
            return schedule_name(view, step, demote, w);
        }
        let display = w.find_all(id("title", 17));
        let input = w.find_all(id("editor", 17));
        let second = match (&display[..], &input[..]) {
            ([d], []) | ([], [d]) => d,
            _ => panic!("expected one second-row control"),
        };
        assert_eq!(first.len(), 1);
        assert!(
            first[0].visible()
                && second.visible()
                && first[0].bounds().bottom() <= second.bounds().top()
        );
        assert_eq!(first[0].label(), Some("Review"));
        assert!(w.find_all(id("editor", 42)).is_empty());
        let count = view.read(cx).submissions;
        if demote {
            assert!(!message(w, "Saved"));
        }
        let next = match step {
            Start => {
                assert!(message(w, "Board items"));
                assert_eq!(second.label(), Some("Review"));
                assert_eq!(count, 0);
                checkpoint("opened");
                click(second, w, cx);
                Focused
            }
            Focused => {
                assert_eq!(second.focused(), Some(true));
                assert_eq!(second.label(), Some("Review"));
                w.press("f2", cx);
                if demote { EditName } else { EditCancel }
            }
            EditCancel | EditName => {
                editor(second, "Review");
                if matches!(step, EditCancel) {
                    replace("Discard me", w, cx);
                    Cancel
                } else {
                    replace(" Coral ", w, cx);
                    Submit
                }
            }
            Cancel => {
                editor(second, "Discard me");
                w.press("escape", cx);
                Cancelled
            }
            Cancelled | Skipped => {
                assert_eq!(second.label(), Some("Review"));
                assert_eq!(second.focused(), Some(true));
                assert_eq!(count, 0);
                if matches!(step, Cancelled) {
                    checkpoint("cancelled");
                    w.press("f2", cx);
                    Unchanged
                } else {
                    checkpoint("unchanged");
                    w.double_click(id("title", 17), cx);
                    EditName
                }
            }
            Unchanged => {
                editor(second, "Review");
                w.press("enter", cx);
                Skipped
            }
            Submit => {
                editor(second, " Coral ");
                assert_eq!(count, 0);
                checkpoint("draft"); // parent demotes the Member or locks the row
                if demote {
                    w.press("enter", cx);
                    Denied
                } else {
                    click(&first[0], w, cx);
                    Pending
                }
            }
            Pending if message(w, "Saving…") => {
                assert_eq!(second.label(), Some(" Coral "));
                assert_eq!(count, 1);
                checkpoint("pending");
                Saved
            }
            Saved | Reopen if matches!(step, Reopen) || message(w, "Saved") => {
                assert_eq!(second.label(), Some("Coral"));
                assert_eq!(count, usize::from(matches!(step, Saved)));
                if matches!(step, Saved) {
                    assert_eq!(first[0].focused(), Some(true));
                }
                checkpoint(if matches!(step, Saved) {
                    "saved"
                } else {
                    "reopened"
                });
                w.remove_window();
                return;
            }
            Denied if message(w, "Not saved: permission denied.") => {
                assert!(message(w, "Board items · Read only"));
                assert_eq!(second.label(), Some("Review"));
                assert_eq!(count, 1);
                checkpoint("denied");
                w.remove_window();
                return;
            }
            Viewer | AfterDoubleClick | AfterF2 => {
                assert!(message(w, "Board items · Read only"));
                assert_eq!(second.label(), Some("Review"));
                assert_eq!(count, 0);
                match step {
                    Viewer => {
                        w.double_click(id("title", 17), cx);
                        AfterDoubleClick
                    }
                    AfterDoubleClick => {
                        w.press("f2", cx);
                        AfterF2
                    }
                    _ => {
                        checkpoint("read_only");
                        w.remove_window();
                        return;
                    }
                }
            }
            _ => step,
        };
        schedule_name(view, next, demote, w);
    });
}
