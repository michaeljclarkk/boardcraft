//! Normal GPUI event dispatch, with parent-owned HTTP and reconnect checkpoints.
use super::*;
use crate::connected::Connected;
use gpui_kit::test::TestQueryExt;

#[derive(Clone, Copy, Debug)]
enum Choice {
    Cancel,
    Submit,
    Fresh,
}
#[derive(Clone, Copy, Debug)]
enum Step {
    ReopenList,
    List,
    Open,
    Menu(Choice),
    Dialog(Choice),
    Choose(Choice),
    Chosen(Choice),
    Cancelled,
    Checks,
    NameCancel,
    Pending,
    Saved,
    After,
    Back,
    Reopened,
    Failed,
    Reloaded,
    Deliberate,
}
fn hit(w: &Window, id: &str) -> ElementSnapshot {
    w.find_all(SharedString::from(id.to_owned()))[0].clone()
}
fn row(w: &Window, prefix: &str, n: u128) -> ElementSnapshot {
    hit(w, &format!("{prefix}-{}", Uuid::from_u128(n)))
}
fn order(w: &Window, ids: &[u128]) {
    let actual: Vec<_> = w
        .elements()
        .iter()
        .filter_map(|e| {
            let Some(ElementId::Name(id)) = e.path().last() else {
                return None;
            };
            Some(id.strip_prefix("title-")?.parse::<Uuid>().ok()?.as_u128())
        })
        .collect();
    assert_eq!(actual, ids);
    for n in [57, 62, 17, 42] {
        assert!(
            w.find_all(SharedString::from(format!("title-{}", Uuid::from_u128(n))))
                .is_empty()
        );
    }
    for id in ["g-90", "g-20"] {
        assert_eq!(hit(w, &format!("group-name-{id}")).label(), Some("Work"));
    }
}
fn drag(w: &mut Window, after: bool, release: bool, cx: &mut App) {
    let from = row(w, "grip", 67).bounds().center();
    let bounds = row(w, "selected-row", 61).bounds();
    let to = bounds.center() + point(px(0.), if after { px(1.) } else { px(0.) });
    w.pointer_down(from, MouseButton::Left, cx);
    w.pointer_move(to, Some(MouseButton::Left), cx);
    assert_eq!(
        hit(w, "item-order-marker").bounds().top(),
        if after {
            bounds.bottom() - px(1.)
        } else {
            bounds.top()
        }
    );
    if release {
        w.pointer_up(to, MouseButton::Left, cx);
    }
}
fn menu(w: &mut Window, cx: &mut App) {
    click(&row(w, "item-menu", 61), w, cx);
}
fn move_menu_is_open(w: &Window) -> bool {
    w.find_all_by_label("Move to…").iter().any(|e| {
        e.path()
            .iter()
            .any(|id| matches!(id, ElementId::Name(name) if name.as_ref() == "popup-menu"))
    })
}
fn stable_draft_and_selection(w: &Window) {
    assert_eq!(hit(w, "add-g-20").value(), Some(" Other draft "));
    assert_eq!(row(w, "select", 73).checked(), Some(true));
}
fn saved_groups(w: &mut Window, reorder: bool, after: bool) {
    for (group, ids) in [
        (
            "g-90",
            if reorder {
                if after {
                    vec![73, 61, 67, 77]
                } else {
                    vec![73, 67, 61, 77]
                }
            } else {
                vec![73]
            },
        ),
        ("g-10", vec![]),
        ("g-20", if reorder { vec![59] } else { vec![61, 59] }),
    ] {
        let actual: Vec<_> = w
            .within(SharedString::from(format!("group-{group}")))
            .elements()
            .iter()
            .filter_map(|e| {
                let Some(ElementId::Name(id)) = e.path().last() else {
                    return None;
                };
                Some(id.strip_prefix("title-")?.parse::<Uuid>().ok()?.as_u128())
            })
            .collect();
        assert_eq!(actual, ids);
    }
}
pub(crate) fn install(v: Entity<Connected>, mode: &str, w: &Window) {
    schedule(
        v,
        mode.to_owned(),
        if mode.ends_with("reopened") {
            Step::ReopenList
        } else {
            Step::List
        },
        w,
    );
}
fn schedule(v: Entity<Connected>, mode: String, step: Step, w: &Window) {
    w.on_next_frame(move |w, cx| {
        use self::Step::*;
        let reorder = mode.starts_with("--placement-order");
        let happy = mode == "--placement-move" || mode == "--placement-order";
        let initial: &[u128] = if reorder { &[73,61,67,77,59] } else { &[73,61,59] };
        let saved: &[u128] = if reorder { &[73,67,61,77,59] } else { &[73,61,59] };
        let count = v.read(cx).submissions;
        let next = match step {
            ReopenList if message(w, "Boards") => { click(&hit(w, &format!("board-{}", Uuid::from_u128(201))), w, cx); Reopened }
            List if message(w, "Boards") => { click(&hit(w, &format!("board-{}", Uuid::from_u128(201))), w, cx); Open }
            Open if message(w, "Main table") => {
                order(w, initial); checkpoint("opened");
                if happy {
                    click(&hit(w, "add-g-20"), w, cx); w.input(" Other draft ", cx);
                    click(&row(w, "select", 73), w, cx);
                }
                if reorder {
                    if happy { Checks }
                    else {
                        drag(w, false, false, cx); checkpoint("ready");
                        w.pointer_up(w.mouse_position(), MouseButton::Left, cx); Failed
                    }
                } else { menu(w, cx); Menu(if happy { Choice::Cancel } else { Choice::Submit }) }
            }
            Menu(choice) if move_menu_is_open(w) => {
                let command = w.within("popup-menu").find_by_label("Move to…");
                assert_ne!(command.disabled(), Some(true));
                click(&command, w, cx); Dialog(choice)
            }
            Dialog(choice) if !w.find_all("item-move-confirm").is_empty() => {
                let (id, group, _) = v.read(cx).single_move_snapshot(cx).unwrap();
                assert_eq!(id, Uuid::from_u128(61)); assert_eq!(group.as_ref(), "g-90");
                click(&w.find_by_label("Destination group"), w, cx); Choose(choice)
            }
            Choose(choice) if w.find_by_label("Destination group").expanded() == Some(true) => {
                w.press("down", cx); w.press("down", cx); w.press("enter", cx); Chosen(choice)
            }
            Chosen(choice) if w.find_by_label("Destination group").expanded() != Some(true) => {
                let (id, group, _) = v.read(cx).single_move_snapshot(cx).unwrap();
                assert_eq!(id, Uuid::from_u128(61)); assert_eq!(group.as_ref(), "g-20");
                match choice {
                    Choice::Cancel => { w.press("escape", cx); Cancelled }
                    Choice::Submit => { checkpoint("ready"); click(&hit(w, "item-move-confirm"), w, cx); if happy { Pending } else { Failed } }
                    Choice::Fresh => { click(&hit(w, "item-move-confirm"), w, cx); Deliberate }
                }
            }
            Cancelled if w.find_all("item-move-confirm").is_empty() => {
                assert_eq!(count, 0); order(w, initial); stable_draft_and_selection(w);
                assert_eq!(row(w, "item-menu", 61).focused(), Some(true)); checkpoint("cancelled");
                menu(w, cx); Menu(Choice::Submit)
            }
            Checks => {
                let p = row(w, "grip", 67).bounds().center();
                w.pointer_down(p, MouseButton::Left, cx);
                w.pointer_move(p + point(px(3.),px(4.)), Some(MouseButton::Left), cx);
                assert!(!message(w, "Moving item"));
                w.pointer_move(p + point(px(6.),px(0.)), Some(MouseButton::Left), cx);
                assert!(message(w, "Moving item"));
                let target = row(w, "selected-row", 61).bounds();
                w.pointer_move(target.center(), Some(MouseButton::Left), cx);
                assert_eq!(hit(w, "item-order-marker").bounds().top(), target.top());
                w.pointer_move(target.center() + point(px(0.),px(1.)), Some(MouseButton::Left), cx);
                assert_eq!(hit(w, "item-order-marker").bounds().top(), target.bottom() - px(1.));
                w.press("escape", cx); w.pointer_up(w.mouse_position(), MouseButton::Left, cx);
                assert!(w.find_all("item-order-marker").is_empty()); assert_eq!(count,0);
                checkpoint("cancelled");
                // Another group, self, outside and a short grip click cannot submit.
                for to in [row(w,"selected-row",59).bounds().center(), p + point(px(8.),px(0.)), point(px(-10.),px(-10.)), target.center()+point(px(0.),px(1.))] {
                    w.pointer_down(p, MouseButton::Left, cx); w.pointer_move(to, Some(MouseButton::Left), cx); w.pointer_up(to, MouseButton::Left, cx);
                }
                w.pointer_down(p, MouseButton::Left, cx); w.pointer_move(p + point(px(3.),px(4.)), Some(MouseButton::Left), cx); w.pointer_up(p, MouseButton::Left, cx);
                order(w, initial); assert_eq!(v.read(cx).submissions,0); stable_draft_and_selection(w); checkpoint("invalid");
                w.double_click(SharedString::from(format!("title-{}",Uuid::from_u128(67))),cx); NameCancel
            }
            NameCancel if !w.find_all(SharedString::from(format!("editor-{}",Uuid::from_u128(67)))).is_empty() => {
                w.press("escape",cx); assert_eq!(count,0); drag(w,false,false,cx); checkpoint("ready");
                w.pointer_up(w.mouse_position(),MouseButton::Left,cx); Pending
            }
            Pending if count == 1 && message(w, if reorder { "Saving item order…" } else { "Moving item…" }) => {
                order(w,initial);
                if reorder { assert_eq!(row(w,"grip",67).disabled(),Some(true)); }
                else { assert_eq!(hit(w,"item-move-confirm").disabled(),Some(true)); click(&hit(w,"item-move-confirm"),w,cx); }
                let p = row(w,"grip",if reorder {67} else {61}).bounds().center();
                let target = row(w,"selected-row",73).bounds().center();
                w.pointer_down(p,MouseButton::Left,cx); w.pointer_move(target,Some(MouseButton::Left),cx); w.pointer_up(target,MouseButton::Left,cx);
                assert!(w.find_all("item-order-marker").is_empty());
                assert_eq!(v.read(cx).submissions,1); checkpoint("pending"); Saved
            }
            Saved if count == 1 && !message(w,"Saving item order…") && !message(w,"Moving item…") && w.find_all("item-move-confirm").is_empty() => {
                order(w,saved); saved_groups(w,reorder,false); stable_draft_and_selection(w); checkpoint("saved");
                if reorder { drag(w,true,true,cx); After } else { click(&hit(w,"board-list"),w,cx); Back }
            }
            After if count == 2 && !message(w,"Saving item order…") => {
                order(w,initial); saved_groups(w,reorder,true); stable_draft_and_selection(w); checkpoint("after"); click(&hit(w,"board-list"),w,cx); Back
            }
            Back if message(w,"Boards") => { click(&hit(w,&format!("board-{}",Uuid::from_u128(201))),w,cx); Reopened }
            Reopened if message(w,"Main table") => {
                order(w,initial); saved_groups(w,reorder,true); assert!(!message(w,"Moving item")); assert!(w.find_all("item-order-marker").is_empty()); checkpoint("revisited"); w.remove_window(); return;
            }
            Failed if message(w,"Not saved: permission denied.") || message(w,"Destination unavailable") || message(w,"Session expired. Sign in again.") => {
                if mode.ends_with("demoted") { order(w,initial); assert_eq!(row(w,"grip",if reorder {67} else {61}).disabled(),Some(true)); }
                assert_eq!(count,1); checkpoint("denied");
                if mode.ends_with("demoted") { click(&hit(w,"reload-table"),w,cx); Reloaded }
                else { w.remove_window(); return; }
            }
            Failed if message(w,"Group changed. Review move.") || message(w,"Could not confirm move. Review move; it will not be repeated automatically.")
                || message(w,"Move saved. Reload board to confirm accepted placement.") || message(w,"Item order changed. Reload board, then drag again.")
                || message(w,"Item order not confirmed. Reload board; the drop will not be repeated.") => {
                order(w,initial); assert_eq!(count,1); checkpoint("failed");
                if !reorder && !mode.ends_with("refresh") { click(&hit(w,&format!("item-move-review-{}",Uuid::from_u128(61))),w,cx); }
                else { click(&hit(w,"reload-table"),w,cx); }
                Reloaded
            }
            Reloaded if message(w,"Current placement loaded. A read does not repeat the move.") || message(w,"Board reloaded. Review accepted items before making another change.")
                || (!reorder && !mode.ends_with("refresh") && !mode.ends_with("demoted")
                    && w.find_all(SharedString::from(format!("item-move-apply-{}",Uuid::from_u128(61)))).first().is_some_and(|e| e.disabled() != Some(true))) => {
                assert_eq!(count,1); checkpoint("reloaded");
                if reorder { drag(w,mode.ends_with("ack") || mode.ends_with("refresh"),true,cx); Deliberate }
                else if !mode.ends_with("refresh") && !mode.ends_with("demoted") {
                    let group = if mode.ends_with("conflict") { "Current group: Reserve (g-10)" } else if mode.ends_with("ack") { "Current group: Work (g-20)" } else { "Current group: Work (g-90)" };
                    assert!(message(w,group)); assert!(message(w,"Requested group: Work (g-20)"));
                    click(&hit(w,&format!("item-move-apply-{}",Uuid::from_u128(61))),w,cx); Deliberate
                } else { menu(w,cx); Menu(Choice::Fresh) }
            }
            Reloaded if (message(w,"Could not reload board. Retry loading before editing again.") || message(w,"Could not read item placement. Reload board or review again."))
                && hit(w,"reload-table").disabled() != Some(true) => {
                assert_eq!(count,1); checkpoint("read-refused");
                if !reorder && mode.ends_with("ack") { click(&hit(w,&format!("item-move-review-{}",Uuid::from_u128(61))),w,cx); }
                else { click(&hit(w,"reload-table"),w,cx); }
                Reloaded
            }
            Deliberate if count == 2 && !message(w,"Saving item order…") && !message(w,"Moving item…") && w.find_all("item-move-confirm").is_empty() => {
                assert!(w.find_all("item-order-marker").is_empty());
                if reorder && mode.ends_with("conflict") { assert_eq!(row(w,"title",67).label(),Some("Indigo")); }
                checkpoint("deliberate"); w.remove_window(); return;
            }
            _ => step,
        };
        schedule(v,mode,next,w);
    });
}
