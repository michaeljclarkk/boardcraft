//! Fixed acceptance journey in this process's own, normally presented Kit window.
use super::*;
use gpui_kit::test::{ElementSnapshot, TestEventExt, TestWindowExt};

#[derive(Debug, Copy, Clone)]
enum Step {
    Open,
    Focus,
    Blank,
    Rejected,
    Corrected,
    Pending,
    Saved,
    Reopen,
}

pub(super) fn install(window: AnyWindowHandle, board: Entity<Workbench>, cx: &mut App) {
    let step = match std::env::args().nth(2).as_deref() {
        Some("--smoke-edit") => Step::Open,
        Some("--smoke-reopen") => Step::Reopen,
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

fn schedule(step: Step, board: Entity<Workbench>, w: &Window) {
    w.on_next_frame(move |w, cx| {
        let view = board.read(cx);
        assert!(!view.failed, "{}", view.message);
        // Read the previous completed native frame. Never set an input, phase or focus.
        let titles = w.find_all("title");
        let [first, second] = &titles[..] else {
            assert!(titles.is_empty(), "expected two title controls");
            return schedule(step, board, w);
        };
        assert!(
            first.visible() && second.visible() && first.bounds().bottom() <= second.bounds().top()
        );
        assert_eq!(first.value(), Some("Review"));
        let rows = &view.rows;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].read(cx).edit.phase(), &Phase::Clean);
        let row = rows[1].read(cx);
        let next = match step {
            Step::Open => {
                assert_eq!(second.value(), Some("Review"));
                click(second, w, cx);
                Step::Focus
            }
            Step::Focus => {
                assert_eq!(second.focused(), Some(true));
                w.press("ctrl-a", cx);
                w.input("   ", cx);
                Step::Blank
            }
            Step::Blank => {
                assert_eq!(second.value(), Some("   "));
                assert_eq!(row.edit.draft(), "   ");
                w.press("enter", cx);
                Step::Rejected
            }
            Step::Rejected if message(w, "Not saved: enter a title.") => {
                assert_eq!(
                    row.edit.phase(),
                    &Phase::Failed(Failure::Rejected(RenameError::BlankTitle))
                );
                assert_eq!(row.edit.accepted(), "Review");
                assert_eq!(row.edit.draft(), "   ");
                assert_eq!(second.value(), Some("   "));
                assert_eq!(second.focused(), Some(true));
                assert_eq!(w.find_all("save")[1].label(), Some("Retry"));
                println!("blank rejected; exact draft and focus retained");
                w.press("ctrl-a", cx);
                w.input(" Review artwork ", cx);
                Step::Corrected
            }
            Step::Corrected if message(w, "Unsaved draft") => {
                assert_eq!(second.value(), Some(" Review artwork "));
                let save = &w.find_all("save")[1];
                assert_eq!(save.label(), Some("Save"));
                click(save, w, cx);
                Step::Pending
            }
            Step::Pending if message(w, "Saving…") => {
                assert_eq!(row.edit.phase(), &Phase::Pending);
                assert_eq!(row.edit.accepted(), "Review");
                assert_eq!(w.find_all("save")[1].disabled(), Some(true));
                // The test parent releases its row lock only after this frame.
                println!("pending frame");
                Step::Saved
            }
            Step::Saved if message(w, "Saved") => {
                assert_eq!(row.edit.phase(), &Phase::Saved);
                assert_eq!(row.edit.accepted(), "Review artwork");
                assert_eq!(second.value(), Some("Review artwork"));
                println!("saved frame; closing");
                w.remove_window();
                return;
            }
            Step::Reopen => {
                assert_eq!(second.value(), Some("Review artwork"));
                assert_eq!(row.edit.phase(), &Phase::Clean);
                println!("reopened ordered titles; closing");
                w.remove_window();
                return;
            }
            _ => step,
        };
        schedule(next, board, w);
    });
}
