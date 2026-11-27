//! Live Kit input; the parent checks payloads and independent database snapshots.
use super::*;
use crate::connected::Connected;
use gpui_kit::test::TestQueryExt;

const A: &str = "g-old";
const B: &str = "g-z";
const C: &str = "g-end";
fn hit(w: &Window, id: &str) -> ElementSnapshot {
    w.find_all(SharedString::from(id.to_owned()))[0].clone()
}
fn name(w: &Window, id: &str) -> ElementSnapshot {
    hit(w, &format!("group-name-{id}"))
}
fn menu(w: &mut Window, id: &str, cx: &mut App) {
    click(&hit(w, &format!("group-menu-{id}")), w, cx);
}
fn order(w: &Window, ids: &[&str]) {
    let names: Vec<_> = w
        .elements()
        .iter()
        .filter_map(|e| {
            let Some(ElementId::Name(n)) = e.path().last() else {
                return None;
            };
            n.strip_prefix("group-name-").map(str::to_owned)
        })
        .collect();
    assert_eq!(names, ids);
    assert_eq!(name(w, A).label(), Some("Work"));
    assert_eq!(name(w, B).label(), Some("Work"));
    assert_eq!(name(w, C).label(), Some("Reserve"));
    assert_eq!(hit(w, "fold-g-old").expanded(), Some(true));
    assert_eq!(hit(w, "fold-g-z").expanded(), Some(false));
    for text in ["1 item", "2 items", "0 items"] {
        assert!(message(w, text));
    }
    assert_eq!(
        hit(w, &format!("title-{}", Uuid::from_u128(59))).label(),
        Some("Ink")
    );
    for n in [61, 63, 67, 57, 62] {
        assert!(
            w.find_all(SharedString::from(format!("title-{}", Uuid::from_u128(n))))
                .is_empty()
        );
    }
    assert!(w.find_all("group-field-name").is_empty());
}
#[derive(Clone, Copy)]
enum Step {
    List,
    Open,
    Menu,
    Boundary,
    Up,
    Start,
    Threshold,
    Cancel,
    Invalid,
    Before,
    DropBefore,
    Saved,
    Resize,
    Scroll(std::time::Instant, Pixels),
    ScrollCancel(std::time::Instant, Pixels),
    Restore,
    After,
    Revisit,
    Reopened,
    Failed,
    Reload,
    FreshMenu,
    Fresh,
    ReadFault,
    Disabled,
}
pub(crate) fn install(v: Entity<Connected>, mode: &str, w: &Window) {
    schedule(v, mode.to_owned(), Step::List, w);
}
fn schedule(v: Entity<Connected>, mode: String, step: Step, w: &Window) {
    w.on_next_frame(move |w, cx| {
        use self::Step::*;
        let happy = mode == "--order-moves";
        let count = v.read(cx).submissions;
        let next = match step {
            List if message(w, "Boards") => {
                click(&hit(w, &format!("board-{}", Uuid::from_u128(201))), w, cx);
                Open
            }
            Open if message(w, "Main table") => {
                order(w, &[A, B, C]);
                checkpoint("opened");
                if mode.contains("conflict") || mode.contains("demoted") || mode.contains("refresh")
                {
                    let p = name(w, B).bounds().center();
                    let to = name(w, A).bounds().center();
                    w.pointer_down(p, MouseButton::Left, cx);
                    w.pointer_move(to, Some(MouseButton::Left), cx);
                    assert!(message(w, "Moving Work"));
                    checkpoint("ready");
                    w.pointer_up(to, MouseButton::Left, cx);
                    Failed
                } else {
                    if happy {
                        click(&hit(w, "add-g-end"), w, cx);
                        w.input(" Other draft ", cx);
                    }
                    menu(w, if happy { A } else { B }, cx);
                    Menu
                }
            }
            Menu if w.try_find_by_label("Move group up").is_some() => {
                if happy {
                    // First boundary, dismissing and a disabled activation write nothing.
                    click(&w.find_by_label("Move group up"), w, cx);
                    assert_eq!(count, 0);
                    w.press("escape", cx);
                    Boundary
                } else if mode.contains("member") || mode.contains("viewer") {
                    click(&w.find_by_label("Move group up"), w, cx);
                    click(&w.find_by_label("Move group down"), w, cx);
                    w.press("escape", cx);
                    let p = name(w, C).bounds().center();
                    w.pointer_down(p, MouseButton::Left, cx);
                    w.pointer_move(name(w, A).bounds().center(), Some(MouseButton::Left), cx);
                    w.pointer_up(name(w, A).bounds().center(), MouseButton::Left, cx);
                    Disabled
                } else {
                    checkpoint("ready");
                    click(&w.find_by_label("Move group up"), w, cx);
                    Failed
                }
            }
            Boundary if w.try_find_by_label("Move group up").is_none() => {
                menu(w, B, cx);
                Up
            }
            Up if w.try_find_by_label("Move group up").is_some() => {
                checkpoint("ready");
                click(&w.find_by_label("Move group up"), w, cx);
                Start
            }
            Start if count == 1 && message(w, "Saving group order…") => {
                order(w, &[A, B, C]);
                assert_eq!(hit(w, "group-menu-g-z").disabled(), Some(true));
                checkpoint("pending");
                Threshold
            }
            Threshold if count == 1 && !message(w, "Saving group order…") => {
                order(w, &[B, A, C]);
                checkpoint("menu-saved");
                let p = name(w, C).bounds().center();
                w.pointer_down(p, MouseButton::Left, cx);
                w.pointer_move(p + point(px(3.), px(4.)), Some(MouseButton::Left), cx);
                assert!(!message(w, "Moving Reserve"));
                w.pointer_move(p + point(px(6.), px(0.)), Some(MouseButton::Left), cx);
                assert!(message(w, "Moving Reserve"));
                let target = name(w, B).bounds().center();
                w.pointer_move(target, Some(MouseButton::Left), cx);
                assert_eq!(
                    hit(w, "group-order-marker").bounds().top(),
                    hit(w, "fold-g-z").bounds().top()
                );
                w.pointer_move(target + point(px(0.), px(1.)), Some(MouseButton::Left), cx);
                assert_eq!(
                    hit(w, "group-order-marker").bounds().top(),
                    hit(w, "fold-g-z").bounds().bottom()
                );
                w.press("escape", cx);
                w.pointer_up(name(w, B).bounds().center(), MouseButton::Left, cx);
                Cancel
            }
            Cancel => {
                assert!(!message(w, "Moving Reserve"));
                assert_eq!(count, 1);
                order(w, &[B, A, C]);
                assert_eq!(hit(w, "add-g-end").value(), Some(" Other draft "));
                // Already-adjacent, self and non-name starts do not become group writes.
                let p = name(w, C).bounds().center();
                let to = name(w, A).bounds().center() + point(px(0.), px(1.));
                w.pointer_down(p, MouseButton::Left, cx);
                w.pointer_move(to, Some(MouseButton::Left), cx);
                w.pointer_up(to, MouseButton::Left, cx);
                let p = name(w, B).bounds().center();
                let to = p + point(px(8.), px(0.));
                w.pointer_down(p, MouseButton::Left, cx);
                w.pointer_move(to, Some(MouseButton::Left), cx);
                w.pointer_up(to, MouseButton::Left, cx);
                let p = hit(w, "fold-g-old").bounds().center();
                w.pointer_down(p, MouseButton::Left, cx);
                w.pointer_move(name(w, B).bounds().center(), Some(MouseButton::Left), cx);
                w.pointer_up(point(px(-10.), px(-10.)), MouseButton::Left, cx);
                assert_eq!(v.read(cx).submissions, 1);
                assert!(!message(w, "Moving Work"));
                checkpoint("cancelled");
                let p = name(w, C).bounds().center();
                w.pointer_down(p, MouseButton::Left, cx);
                w.pointer_move(name(w, B).bounds().center(), Some(MouseButton::Left), cx);
                w.pointer_up(point(px(-10.), px(-10.)), MouseButton::Left, cx);
                Invalid
            }
            Invalid => {
                assert_eq!(count, 1);
                order(w, &[B, A, C]);
                checkpoint("invalid");
                // The subsequent deliberate double-click still edits, independent of drag.
                w.double_click("group-name-g-z", cx);
                Before
            }
            Before if !w.find_all("group-field-name").is_empty() => {
                w.press("escape", cx);
                DropBefore
            }
            DropBefore if w.find_all("group-field-name").is_empty() => {
                let p = name(w, C).bounds().center();
                w.pointer_down(p, MouseButton::Left, cx);
                let target = name(w, B).bounds().center();
                w.pointer_move(target, Some(MouseButton::Left), cx);
                w.pointer_up(target, MouseButton::Left, cx);
                Saved
            }
            Saved if count == 2 && !message(w, "Saving group order…") => {
                order(w, &[C, B, A]);
                checkpoint("drag-before");
                w.resize(size(px(800.), px(420.)));
                Resize
            }
            Resize => {
                let p = name(w, C).bounds().center();
                assert!(name(w, C).visible());
                w.pointer_down(p, MouseButton::Left, cx);
                let target = point(p.x, w.visual_viewport_bounds().bottom() - px(8.));
                w.pointer_move(target, Some(MouseButton::Left), cx);
                Scroll(std::time::Instant::now(), name(w, A).bounds().top())
            }
            Scroll(start, original)
                if name(w, A).visible()
                    && name(w, A).bounds().bottom() < w.mouse_position().y
                    && name(w, A).bounds().top() < original =>
            {
                assert!(start.elapsed().as_secs() < 10);
                assert!(name(w, A).bounds().top() < original);
                assert!(message(w, "Moving Reserve"));
                checkpoint("scrolled");
                w.press("escape", cx);
                w.pointer_up(w.mouse_position(), MouseButton::Left, cx);
                ScrollCancel(std::time::Instant::now(), name(w, A).bounds().top())
            }
            Scroll(start, _) if start.elapsed().as_secs() > 10 => {
                panic!("edge scroll did not expose last heading")
            }
            ScrollCancel(start, y) if start.elapsed().as_millis() > 200 => {
                assert_eq!(name(w, A).bounds().top(), y);
                assert_eq!(count, 2);
                assert!(!message(w, "Moving Reserve"));
                assert!(w.find_all("group-order-marker").is_empty());
                checkpoint("scroll-stopped");
                w.resize(size(px(1024.), px(900.)));
                Restore
            }
            Restore if w.visual_viewport_bounds().size.height >= px(800.) => {
                w.render_frame(cx);
                let p = name(w, C).bounds().center();
                w.pointer_down(p, MouseButton::Left, cx);
                let target = name(w, A).bounds().center() + point(px(0.), px(1.));
                w.pointer_move(target, Some(MouseButton::Left), cx);
                assert!(message(w, "Moving Reserve"));
                w.pointer_up(target, MouseButton::Left, cx);
                After
            }
            After if count == 3 && !message(w, "Saving group order…") => {
                order(w, &[B, A, C]);
                assert_eq!(hit(w, "add-g-end").value(), Some(" Other draft "));
                checkpoint("drag-after");
                click(&hit(w, "board-list"), w, cx);
                Revisit
            }
            Revisit if message(w, "Boards") => {
                click(&hit(w, &format!("board-{}", Uuid::from_u128(201))), w, cx);
                Reopened
            }
            Reopened if message(w, "Main table") => {
                order(w, &[B, A, C]);
                checkpoint("revisited");
                w.remove_window();
                return;
            }
            Failed
                if message(w, "Group order changed. Reload board, then choose again.")
                    || message(
                        w,
                        "Group order not confirmed. Reload board; the move will not be repeated.",
                    ) =>
            {
                order(w, &[A, B, C]);
                assert_eq!(count, 1);
                checkpoint("failed");
                click(&hit(w, "reload-table"), w, cx);
                if mode.contains("refresh") || mode.contains("ack") {
                    ReadFault
                } else {
                    Reload
                }
            }
            ReadFault
                if message(
                    w,
                    "Could not reload board. Retry loading before editing again.",
                ) =>
            {
                order(w, &[A, B, C]);
                assert_eq!(count, 1);
                checkpoint("read-refused");
                click(&hit(w, "reload-table"), w, cx);
                Reload
            }
            Reload
                if message(
                    w,
                    "Board reloaded. Review accepted items before making another change.",
                ) =>
            {
                assert_eq!(count, 1);
                order(
                    w,
                    if mode.contains("conflict") {
                        &[A, C, B]
                    } else if mode.contains("commit") {
                        &[A, B, C]
                    } else {
                        &[B, A, C]
                    },
                );
                checkpoint("reloaded");
                if mode.contains("conflict") || mode.contains("commit") {
                    menu(w, B, cx);
                    FreshMenu
                } else {
                    w.remove_window();
                    return;
                }
            }
            FreshMenu if w.try_find_by_label("Move group up").is_some() => {
                click(&w.find_by_label("Move group up"), w, cx);
                Fresh
            }
            Fresh if count == 2 && !message(w, "Saving group order…") => {
                order(
                    w,
                    if mode.contains("conflict") {
                        &[A, B, C]
                    } else {
                        &[B, A, C]
                    },
                );
                checkpoint("deliberate");
                w.remove_window();
                return;
            }
            Failed
                if message(w, "Not saved: permission denied.")
                    || message(w, "Destination unavailable")
                    || message(w, "Session expired. Sign in again.") =>
            {
                assert_eq!(count, 1);
                if mode.contains("demoted") {
                    order(w, &[A, B, C]);
                    assert_eq!(name(w, B).disabled(), Some(true));
                }
                checkpoint("denied");
                w.remove_window();
                return;
            }
            Disabled => {
                order(w, &[A, B, C]);
                assert_eq!(count, 0);
                checkpoint("disabled");
                w.remove_window();
                return;
            }
            _ => step,
        };
        schedule(v, mode, next, w);
    });
}
