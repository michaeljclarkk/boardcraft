//! Real Kit menu/dialog buttons, Escape and keyboard confirmation; parent owns the oracle.
use super::*;
use crate::connected::Connected;
use gpui_kit::test::TestQueryExt;

#[derive(Clone, Copy, Debug)]
enum Step {
    List,
    Open,
    RootMenu,
    CancelDialog,
    Cancelled,
    RootMenuAgain,
    Confirm,
    Keyboard(usize),
    Pending,
    RootSaved,
    GroupMenu,
    GroupCancelled,
    GroupMenuAgain,
    GroupPending,
    GroupSaved,
    Dismissed,
    Back,
    Revisit,
    Inventory,
    GroupRestored,
    RootRestored,
    ActiveRestored,
    UndoMenu,
    UndoReady,
    Undone,
    Reloaded,
    Refused,
    Reviewed,
    Restored,
    Denied,
    Unavailable,
    Expired,
    Uncertain,
    Reconciled,
}
fn hit(w: &Window, id: &str) -> ElementSnapshot {
    w.find_all(SharedString::from(id.to_owned()))[0].clone()
}
fn item(prefix: &str, n: u128) -> String {
    format!("{prefix}-{}", Uuid::from_u128(n))
}
fn archive_menu(w: &Window) -> Option<ElementSnapshot> {
    w.elements()
        .iter()
        .find(|e| {
            e.label() == Some("Archive…")
                && e.path().contains(&ElementId::Name("popup-menu".into()))
        })
        .cloned()
}
fn order(w: &Window, expected: &[u128]) {
    let actual: Vec<_> = w
        .elements()
        .iter()
        .filter_map(|e| {
            let Some(ElementId::Name(n)) = e.path().last() else {
                return None;
            };
            Some(n.strip_prefix("title-")?.parse::<Uuid>().ok()?.as_u128())
        })
        .collect();
    assert_eq!(actual, expected);
}
fn inventory(w: &Window, grouped: bool, own_archived: bool, owner: bool, ready: bool) {
    assert!(message(w, "Archive"));
    let expected: &[u128] = if grouped {
        &[57, 73, 69, 61, 67, 77]
    } else if own_archived {
        &[57, 67]
    } else {
        &[57]
    };
    let actual: Vec<_> = w
        .elements()
        .iter()
        .filter_map(|e| {
            let Some(ElementId::Name(n)) = e.path().last() else {
                return None;
            };
            Some(
                n.strip_prefix("restore-item-")?
                    .parse::<Uuid>()
                    .ok()?
                    .as_u128(),
            )
        })
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(!w.find_all("restore-group-g-90").is_empty(), grouped);
    assert_eq!(
        hit(w, "restore-group-g-30").disabled() == Some(true),
        !owner || !ready
    );
    for &n in expected {
        assert_eq!(
            hit(w, &item("restore-item", n)).disabled() == Some(true),
            n == 57 || grouped || !owner || !ready
        );
    }
    assert!(message(w, "Restore its group first."));
    assert!(w.find_all(item("restore-item", 68)).is_empty());
    assert!(w.find_all(item("restore-item", 62)).is_empty());
}
pub(crate) fn install(v: Entity<Connected>, mode: &str, w: &Window) {
    schedule(v, mode.to_owned(), Step::List, w);
}
fn schedule(v: Entity<Connected>, mode: String, step: Step, w: &Window) {
    w.on_next_frame(move |w,cx| {
        use self::Step::*;
        let happy = mode == "--archive-round-trip";
        let count = v.read(cx).submissions;
        let next = match step {
            List if message(w,"Boards") => { click(&hit(w,&item("board",201)),w,cx); Open }
            Open if message(w,"Main table") => {
                order(w,if happy { &[59,73,61,67,77] } else { &[59] });
                checkpoint("opened");
                if happy { click(&hit(w,&item("item-menu",67)),w,cx); RootMenu }
                else { click(&hit(w,"open-archive"),w,cx); Inventory }
            }
            RootMenu if archive_menu(w).is_some() => { click(&archive_menu(w).unwrap(),w,cx); CancelDialog }
            CancelDialog if !w.find_all("archive-confirm").is_empty() => {
                assert!(message(w,"Restore this item later from board Archive.")); assert!(message(w,"Ink"));
                checkpoint("cancel-ready"); w.press("escape",cx); Cancelled
            }
            Cancelled if w.find_all("archive-confirm").is_empty() => {
                assert_eq!(count,0); assert_eq!(hit(w,&item("item-menu",67)).focused(),Some(true));
                checkpoint("cancelled"); click(&hit(w,&item("item-menu",67)),w,cx); RootMenuAgain
            }
            RootMenuAgain if archive_menu(w).is_some() => { click(&archive_menu(w).unwrap(),w,cx); Confirm }
            Confirm if !w.find_all("archive-confirm").is_empty() => { checkpoint("confirming"); Keyboard(0) }
            Keyboard(tabs) => {
                if hit(w,"archive-confirm").focused()==Some(true) { w.press("enter",cx); Pending }
                else { assert!(tabs<8); w.press("tab",cx); Keyboard(tabs+1) }
            }
            Pending if count==1 && hit(w,"archive-confirm").disabled()==Some(true) => {
                order(w,&[59,73,61,67,77]); click(&hit(w,"archive-confirm"),w,cx); assert_eq!(v.read(cx).submissions,1);
                checkpoint("pending"); RootSaved
            }
            RootSaved if w.find_all("archive-confirm").is_empty() && !message(w,"Saving archive change…") => {
                order(w,&[59,73,61,77]); checkpoint("root-archived"); click(&hit(w,"group-menu-g-90"),w,cx); GroupMenu
            }
            GroupMenu if w.try_find_by_label("Archive group").is_some() => { w.press("escape",cx); GroupCancelled }
            GroupCancelled if w.try_find_by_label("Archive group").is_none() => {
                assert_eq!(count,1); checkpoint("group-cancelled"); click(&hit(w,"group-menu-g-90"),w,cx); GroupMenuAgain
            }
            GroupMenuAgain if w.try_find_by_label("Archive group").is_some() => { checkpoint("group-confirming"); click(&w.find_by_label("Archive group"),w,cx); GroupPending }
            GroupPending if count==2 && message(w,"Saving archive change…") => {
                order(w,&[59,73,61,77]); assert_eq!(hit(w,"group-menu-g-90").disabled(),Some(true)); checkpoint("group-pending"); GroupSaved
            }
            GroupSaved if !w.find_all("archive-undo").is_empty() => {
                order(w,&[59]); assert!(message(w,"Work was removed.")); checkpoint("group-archived"); click(&hit(w,"archive-dismiss"),w,cx); Dismissed
            }
            Dismissed if w.find_all("archive-undo").is_empty() => { checkpoint("dismissed"); click(&hit(w,"board-list"),w,cx); Back }
            Back if message(w,"Boards") => { click(&hit(w,&item("board",201)),w,cx); Revisit }
            Revisit if message(w,"Main table") => {
                order(w,&[59]); assert!(w.find_all("archive-undo").is_empty()); checkpoint("revisited"); click(&hit(w,"open-archive"),w,cx); Inventory
            }
            Inventory if message(w,"Archive reloaded. No change was submitted.") => {
                let owner = mode != "--archive-member" && mode != "--archive-viewer";
                inventory(w,true,true,owner,true); checkpoint("inventory");
                if !owner { assert_eq!(count,0); w.remove_window(); return; }
                click(&hit(w,"restore-group-g-90"),w,cx);
                if happy { GroupRestored } else { match mode.as_str() {
                    "--archive-conflict" => Refused,
                    "--archive-demoted" => Denied,
                    "--archive-denied" => Unavailable,
                    "--archive-session" => Expired,
                    _ => Uncertain,
                }}
            }
            GroupRestored if message(w,"2 archived items") && !message(w,"Saving archive change…") => {
                inventory(w,false,true,true,true); assert_eq!(count,3); checkpoint("group-restored"); click(&hit(w,&item("restore-item",67)),w,cx); RootRestored
            }
            RootRestored if message(w,"1 archived items") && !message(w,"Saving archive change…") => {
                inventory(w,false,false,true,true); assert_eq!(count,4); checkpoint("item-restored"); click(&hit(w,"archive-close"),w,cx); ActiveRestored
            }
            ActiveRestored if message(w,"Main table") => {
                order(w,&[59,73,61,67,77]); checkpoint("active-restored"); click(&hit(w,"group-menu-g-90"),w,cx); UndoMenu
            }
            UndoMenu if w.try_find_by_label("Archive group").is_some() => { click(&w.find_by_label("Archive group"),w,cx); UndoReady }
            UndoReady if !w.find_all("archive-undo").is_empty() => {
                order(w,&[59]); assert_eq!(count,5); checkpoint("undo-ready"); click(&hit(w,"archive-undo"),w,cx); Undone
            }
            Undone if w.find_all("archive-undo").is_empty() && !message(w,"Saving archive change…") => {
                order(w,&[59,73,61,67,77]); assert_eq!(count,6); checkpoint("undone"); click(&hit(w,"reload-table"),w,cx); Reloaded
            }
            Reloaded if message(w,"Board reloaded. Review accepted items before making another change.") => {
                order(w,&[59,73,61,67,77]); assert_eq!(count,6); checkpoint("reloaded"); w.remove_window(); return;
            }
            Refused if message(w,"Content changed or is unavailable. Reload Archive before choosing again.") => {
                inventory(w,true,true,true,false); assert_eq!(hit(w,"restore-group-g-90").disabled(),Some(true)); checkpoint("refused"); click(&hit(w,"archive-reload"),w,cx); Reviewed
            }
            Reviewed if message(w,"Archive reloaded. No change was submitted.") => {
                inventory(w,true,true,true,true); assert_eq!(count,1); checkpoint("reviewed"); click(&hit(w,"restore-group-g-90"),w,cx); Restored
            }
            Restored if message(w,"2 archived items") && !message(w,"Saving archive change…") => {
                inventory(w,false,true,true,true); assert_eq!(count,2); checkpoint("restored"); w.remove_window(); return;
            }
            Denied if message(w,"Archive reloaded. No change was submitted.") && message(w,"Only an Owner can archive or restore content.") => {
                inventory(w,true,true,false,true); assert_eq!(count,1); checkpoint("denied"); w.remove_window(); return;
            }
            Unavailable if message(w,"Destination unavailable") => { assert!(w.find_all("restore-group-g-90").is_empty()); checkpoint("unavailable"); w.remove_window(); return; }
            Expired if message(w,"Session expired. Sign in again.") => { assert!(w.find_all("restore-group-g-90").is_empty()); assert!(!w.find_all("identifier").is_empty()); checkpoint("expired"); w.remove_window(); return; }
            Uncertain if message(w,"Archive change not confirmed. Reload Archive; this request will not be repeated.") => {
                assert_eq!(count,1); assert_eq!(hit(w,"restore-group-g-90").disabled(),Some(true)); checkpoint("uncertain"); click(&hit(w,"archive-reload"),w,cx); Reconciled
            }
            Reconciled if message(w,"Archive reloaded. No change was submitted.") => {
                inventory(w,false,true,true,true); assert_eq!(count,1); checkpoint("reconciled"); w.remove_window(); return;
            }
            _ => step,
        };
        schedule(v,mode,next,w);
    });
}
