//! Input is sent to the mounted table, never to its operation methods.
use super::*;
use crate::connected::Connected;
use gpui_kit::test::TestQueryExt;

const A: &str = "g-old";
const B: &str = "g-z";
const C: &str = "g-end";

#[derive(Clone, Copy)]
enum Action {
    Pointer(&'static str),
    Keyboard(&'static str),
    Menu(&'static str, &'static str),
    Back,
    Reload,
    Close,
}

#[derive(Clone, Copy)]
struct Check {
    folds: &'static [&'static str],
    checkpoint: &'static str,
    focus: Option<&'static str>,
    submissions: usize,
    action: Action,
}

fn plan(mode: &str) -> Vec<Check> {
    use self::Action::*;
    let check = |folds, checkpoint, focus, submissions, action| Check {
        folds,
        checkpoint,
        focus,
        submissions,
        action,
    };
    match mode {
        "--table-member" => vec![
            check(&[], "opened", None, 0, Keyboard(B)),
            check(&[B], "folded-b", Some(B), 1, Menu(A, "Collapse all groups")),
            check(
                &[A, B, C],
                "collapsed-all",
                Some(A),
                2,
                Menu(B, "Expand all groups"),
            ),
            check(&[], "expanded-all", Some(B), 3, Pointer(C)),
            check(&[C], "folded-empty", Some(C), 4, Menu(B, "Collapse group")),
            check(
                &[B, C],
                "menu-collapsed",
                Some(B),
                5,
                Menu(B, "Expand group"),
            ),
            check(&[C], "menu-expanded", Some(B), 6, Pointer(B)),
            check(&[B, C], "shared", Some(B), 7, Close),
        ],
        "--table-reopen" => vec![check(&[B, C], "reopened", None, 0, Close)],
        "--table-viewer" => vec![
            check(&[B, C], "viewer-opened", None, 0, Pointer(B)),
            check(
                &[C],
                "viewer-expanded",
                Some(B),
                0,
                Menu(A, "Collapse all groups"),
            ),
            check(
                &[A, B, C],
                "viewer-all",
                Some(A),
                0,
                Menu(B, "Expand all groups"),
            ),
            check(&[], "viewer-open", Some(B), 0, Back),
            check(&[B, C], "viewer-revisited", None, 0, Close),
        ],
        "--table-conflict" => vec![
            check(&[], "rendered", None, 0, Keyboard(B)),
            check(&[C], "reviewed", None, 1, Pointer(B)),
            check(&[B, C], "deliberate", Some(B), 2, Reload),
        ],
        "--table-refresh-fault" | "--table-ack-fault" => vec![
            check(&[], "rendered", None, 0, Pointer(B)),
            check(&[B], "reviewed", None, 1, Pointer(C)),
            check(&[B, C], "deliberate", Some(C), 2, Close),
        ],
        "--table-commit-fault" => vec![
            check(&[], "rendered", None, 0, Pointer(B)),
            check(&[], "reviewed", None, 1, Pointer(B)),
            check(&[B], "deliberate", Some(B), 2, Close),
        ],
        "--table-demoted" => vec![
            check(&[], "rendered", None, 0, Pointer(B)),
            check(&[], "reviewed", None, 1, Pointer(B)),
            check(&[B], "temporary", Some(B), 1, Reload),
            check(&[], "discarded", None, 1, Close),
        ],
        _ => unreachable!(),
    }
}

#[derive(Clone, Copy)]
enum Step {
    List(usize),
    Ready(usize),
    Keyboard(usize, usize),
    Menu(usize, &'static str),
    Recovery(usize),
    Pending(usize),
    ConfirmMenu(usize, &'static str),
}

pub(crate) fn install(view: Entity<Connected>, mode: &str, w: &Window) {
    schedule(view, mode.to_owned(), Step::List(0), w);
}

fn matches_table(w: &Window, folds: &[&str]) -> bool {
    !message(w, "Saving group presentation…")
        && [A, B, C].into_iter().all(|id| {
            w.find_all(SharedString::from(format!("fold-{id}")))
                .first()
                .is_some_and(|h| h.expanded() == Some(!folds.contains(&id)))
        })
}

fn table(w: &Window, folds: &[&str], focus: Option<&str>) {
    assert!(message(w, "Main table"));
    let mut bottom = None;
    let mut left = None;
    for (id, name, count, rows) in [
        (A, "Work", "1 item", &[59][..]),
        (B, "Work", "2 items", &[61, 63][..]),
        (C, "Reserve", "0 items", &[][..]),
    ] {
        let h = &w.find_all(SharedString::from(format!("fold-{id}")))[0];
        assert!(h.visible());
        assert_eq!(h.label(), Some(format!("Toggle {name}").as_str()));
        assert_eq!(h.expanded(), Some(!folds.contains(&id)));
        assert!(bottom.is_none_or(|b| b <= h.bounds().top()));
        assert!(left.is_none_or(|x| x == h.bounds().left()));
        left = Some(h.bounds().left());
        bottom = Some(h.bounds().bottom());
        assert!(message(w, count));
        assert_eq!(
            w.find_all(SharedString::from(format!("group-menu-{id}")))
                .len(),
            1
        );
        if focus == Some(id) {
            assert_eq!(h.focused(), Some(true));
        }
        for n in rows {
            let r = w.find_all(navigation::id("title", *n));
            assert_eq!(r.len(), usize::from(!folds.contains(&id)));
            if let Some(r) = r.first() {
                assert!(r.visible());
                assert_eq!(r.label(), Some("Ink"));
                assert!(bottom.unwrap() <= r.bounds().top());
                bottom = Some(r.bounds().bottom());
            }
        }
    }
    for n in [57, 67, 62, 73, 42, 17] {
        assert!(w.find_all(navigation::id("title", n)).is_empty());
    }
    for n in [59, 61, 63] {
        assert!(w.find_all(navigation::id("editor", n)).is_empty());
    }
}

fn no_table(w: &Window) {
    for id in [A, B, C] {
        assert!(
            w.find_all(SharedString::from(format!("fold-{id}")))
                .is_empty()
        );
    }
    for n in [59, 61, 63] {
        assert!(w.find_all(navigation::id("title", n)).is_empty());
    }
}

fn schedule(view: Entity<Connected>, mode: String, step: Step, w: &Window) {
    w.on_next_frame(move |w, cx| {
        let plan = plan(&mode);
        let next = match step {
            Step::List(ix) if message(w, "Boards") => {
                click(&w.find_all(navigation::id("board", 201))[0], w, cx);
                Step::Ready(ix)
            }
            Step::Ready(ix) if plan.get(ix).is_some_and(|c| matches_table(w, c.folds)) => {
                let check = plan[ix];
                table(w, check.folds, check.focus);
                assert_eq!(view.read(cx).submissions, check.submissions);
                checkpoint(check.checkpoint);
                match check.action {
                    Action::Pointer(id) => {
                        click(
                            &w.find_all(SharedString::from(format!("fold-{id}")))[0],
                            w,
                            cx,
                        );
                        if ix == 0 && mode != "--table-member" && mode != "--table-viewer" {
                            Step::Recovery(ix + 1)
                        } else {
                            Step::Ready(ix + 1)
                        }
                    }
                    Action::Keyboard(_) => Step::Keyboard(ix, 0),
                    Action::Menu(id, label) => {
                        click(
                            &w.find_all(SharedString::from(format!("group-menu-{id}")))[0],
                            w,
                            cx,
                        );
                        Step::Menu(ix, label)
                    }
                    Action::Back => {
                        click(&w.find_all("board-list")[0], w, cx);
                        Step::List(ix + 1)
                    }
                    Action::Reload => {
                        click(&w.find_all("reload-table")[0], w, cx);
                        Step::Ready(ix + 1)
                    }
                    Action::Close => {
                        w.remove_window();
                        return;
                    }
                }
            }
            Step::Keyboard(ix, tabs) => {
                let Action::Keyboard(id) = plan[ix].action else {
                    unreachable!();
                };
                if w.find_all(SharedString::from(format!("fold-{id}")))[0].focused() == Some(true) {
                    w.press("space", cx);
                    if mode == "--table-conflict" {
                        Step::Recovery(ix + 1)
                    } else if mode == "--table-member" {
                        Step::Pending(ix + 1)
                    } else {
                        Step::Ready(ix + 1)
                    }
                } else {
                    assert!(tabs < 16, "disclosure is not keyboard reachable");
                    w.press("tab", cx);
                    Step::Keyboard(ix, tabs + 1)
                }
            }
            Step::Menu(ix, label) if w.try_find_by_label(label).is_some() => {
                let item = w.find_by_label(label);
                assert_eq!(item.role(), Some(Role::MenuItem));
                if label == "Expand group" {
                    w.press("down", cx);
                    Step::ConfirmMenu(ix, label)
                } else {
                    click(&item, w, cx);
                    Step::Ready(ix + 1)
                }
            }
            Step::ConfirmMenu(ix, label) => {
                assert_eq!(w.find_by_label(label).selected(), Some(true));
                w.press("enter", cx);
                Step::Ready(ix + 1)
            }
            Step::Pending(ix) if message(w, "Saving group presentation…") => {
                table(w, &[], Some(B));
                click(
                    &w.find_all(SharedString::from(format!("fold-{B}")))[0],
                    w,
                    cx,
                );
                assert_eq!(view.read(cx).submissions, 1);
                checkpoint("pending");
                Step::Ready(ix)
            }
            Step::Recovery(ix)
                if !w.find_all("reload-table").is_empty() && !message(w, "Main table") =>
            {
                no_table(w);
                assert_eq!(w.find_all("reload-table")[0].focused(), Some(true));
                assert_eq!(view.read(cx).submissions, 1);
                if mode == "--table-conflict" {
                    assert!(message(
                        w,
                        "Group presentation changed. Reload board, then choose again."
                    ));
                }
                checkpoint("refused");
                click(&w.find_all("reload-table")[0], w, cx);
                Step::Ready(ix)
            }
            Step::Ready(_)
                if message(w, "Destination unavailable") && mode == "--table-conflict" =>
            {
                no_table(w);
                assert_eq!(view.read(cx).submissions, 2);
                checkpoint("access-denied");
                w.remove_window();
                return;
            }
            _ => step,
        };
        schedule(view, mode, next, w);
    });
}
