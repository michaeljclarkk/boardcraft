//! Actual toolbar, Kit dialog, review and recovery controls; no direct mutations.
use super::*;
use crate::connected::Connected;
use gpui_kit::test::TestQueryExt;

#[derive(Clone, Copy)]
enum Step {
    List,
    Open,
    Second,
    Third,
    Selected,
    Folded,
    Cancel,
    Cancelled,
    Reopened,
    Keyboard(usize),
    Pending,
    Saved,
    Inventory,
    Restored,
    Visible,
    Reloaded,
    Refused,
    ReviewFailed,
    Reviewed,
    Adopted,
    Deliberate,
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
fn order(w: &Window, ids: &[u128]) {
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
    assert_eq!(actual, ids);
}
pub(crate) fn install(v: Entity<Connected>, mode: &str, w: &Window) {
    schedule(v, mode.to_owned(), Step::List, w);
}
fn schedule(v: Entity<Connected>, mode: String, step: Step, w: &Window) {
    w.on_next_frame(move |w,cx| {
        use self::Step::*;
        let trash=mode.starts_with("--bulk-trash-");
        let happy=mode.ends_with("-success");
        let trigger=if trash {"selection-delete"} else {"selection-archive"};
        let selected=if trash {&[67,59][..]} else {&[67,59,61][..]};
        let selected_label=format!("{} selected",selected.len());
        let saved_order=if trash {&[73,61,77][..]} else {&[73,77][..]};
        let restored_order=if trash {&[73,61,67,77][..]} else {&[73,67,77][..]};
        let next=match step {
            List if message(w,"Boards") => { click(&hit(w,&item("board",201)),w,cx); Open }
            Open if message(w,"Main table") => { order(w,&[59,73,61,67,77]); checkpoint("opened"); click(&hit(w,&item("select",67)),w,cx); Second }
            Second => { click(&hit(w,&item("select",59)),w,cx); if trash {Selected} else {Third} }
            Third => { click(&hit(w,&item("select",61)),w,cx); Selected }
            Selected if message(w,&selected_label) => {
                for n in selected { assert_eq!(hit(w,&item("select",*n)).checked(),Some(true)); }
                if mode.ends_with("-member") || mode.ends_with("-viewer") { assert_eq!(hit(w,trigger).disabled(),Some(true)); click(&hit(w,trigger),w,cx); assert!(w.find_all("move-confirm").is_empty()); checkpoint("readonly"); w.remove_window(); return; }
                click(&hit(w,"fold-g-20"),w,cx); Folded
            }
            Folded if hit(w,"fold-g-20").expanded()==Some(false) && !message(w,"Saving group presentation…") => {
                assert!(w.find_all(item("select",59)).is_empty()); assert!(message(w,&selected_label)); checkpoint("folded"); click(&hit(w,trigger),w,cx); if happy {Cancel} else {Reopened}
            }
            Cancel if !w.find_all("move-confirm").is_empty() => { assert!(message(w,if trash {"Restore these items later from board Trash."} else {"Restore these items later from board Archive."})); checkpoint("cancel-ready"); w.press("escape",cx); Cancelled }
            Cancelled if w.find_all("move-confirm").is_empty() => { assert_eq!(hit(w,trigger).focused(),Some(true)); assert!(message(w,&selected_label)); checkpoint("cancelled"); click(&hit(w,trigger),w,cx); Reopened }
            Reopened if !w.find_all("move-confirm").is_empty() => {
                assert_eq!(hit(w,"move-confirm").label(),Some(if trash {"Delete"} else {"Archive"}));
                assert!(message(w,if trash {"Ink, Ink"} else {"Ink, Ink, Ink"})); assert!(w.try_find_by_label("Destination group").is_none());
                let (_,_,ids)=v.read(cx).selection_move_snapshot(cx).unwrap(); assert_eq!(ids,selected.iter().copied().map(Uuid::from_u128).collect::<Vec<_>>());
                checkpoint("draft"); if happy { Keyboard(0) } else { click(&hit(w,"move-confirm"),w,cx); if mode.ends_with("-demoted") {Denied} else if mode.ends_with("-denied") {Unavailable} else if mode.ends_with("-session") {Expired} else if mode.ends_with("-ack") || mode.ends_with("-refresh") {Uncertain} else {Refused} }
            }
            Keyboard(n) => { if hit(w,"move-confirm").focused()==Some(true) { w.press("enter",cx); Pending } else { assert!(n<10); w.press("tab",cx); Keyboard(n+1) } }
            Pending if message(w,if trash {"Deleting selected items…"} else {"Archiving selected items…"}) => { assert_eq!(hit(w,"move-confirm").disabled(),Some(true)); order(w,&[73,61,67,77]); click(&hit(w,"move-confirm"),w,cx); checkpoint("pending"); Saved }
            Saved if w.find_all("move-confirm").is_empty() && w.find_all("selection-toolbar").is_empty() => { order(w,saved_order); assert_eq!(hit(w,"fold-g-20").expanded(),Some(false)); assert!(message(w,if trash {"3 items"} else {"2 items"})); assert!(message(w,"0 items")); checkpoint("saved"); click(&hit(w,if trash {"open-trash"} else {"open-archive"}),w,cx); Inventory }
            Inventory if message(w,if trash {"3 deleted items"} else {"4 archived items"}) => { for n in if trash {&[68,59,67][..]} else {&[57,59,61,67][..]} { assert!(!w.find_all(item("restore-item",*n)).is_empty()); } checkpoint("inventory"); click(&hit(w,&item("restore-item",67)),w,cx); Restored }
            Restored if message(w,if trash {"2 deleted items"} else {"3 archived items"}) && !message(w,"Saving archive change…") => { assert!(w.find_all(item("restore-item",67)).is_empty()); for n in if trash {&[59,68][..]} else {&[59,61][..]} { assert!(!w.find_all(item("restore-item",*n)).is_empty()); } checkpoint("restored"); click(&hit(w,"archive-close"),w,cx); Visible }
            Visible if message(w,"Main table") => { order(w,restored_order); click(&hit(w,"reload-table"),w,cx); Reloaded }
            Reloaded if message(w,"Board reloaded. Review accepted items before making another change.") => { order(w,restored_order); checkpoint("reloaded"); w.remove_window(); return; }
            Refused if message(w,"Items changed or are unavailable. Review latest items before confirming again.") => { assert_eq!(hit(w,"move-confirm").disabled(),Some(true)); checkpoint("refused"); click(&hit(w,"move-review"),w,cx); if mode.ends_with("-review-failed") {ReviewFailed} else {Reviewed} }
            ReviewFailed if message(w,"Could not review selected items. Retry review.") => { checkpoint("review-failed"); click(&hit(w,"move-review"),w,cx); Reviewed }
            Reviewed if message(w,"Review every requested item before adopting these revisions.") => {
                if mode.ends_with("-missing") { assert_eq!(hit(w,"move-adopt").disabled(),Some(true)); checkpoint("missing-reviewed"); w.press("escape",cx); w.remove_window(); return; }
                assert_ne!(hit(w,"move-adopt").disabled(),Some(true)); checkpoint("reviewed"); click(&hit(w,"move-adopt"),w,cx); Adopted
            }
            Adopted if message(w,if trash {"Revisions adopted. Confirm separately to delete the selected items."} else {"Revisions adopted. Confirm separately to archive the selected items."}) => { assert_ne!(hit(w,"move-confirm").disabled(),Some(true)); checkpoint("adopted"); click(&hit(w,"move-confirm"),w,cx); Deliberate }
            Deliberate if w.find_all("move-confirm").is_empty() => { order(w,saved_order); checkpoint("deliberate"); w.remove_window(); return; }
            Denied if message(w,if trash {"Delete not saved: permission denied."} else {"Archive not saved: permission denied."}) => { assert_eq!(hit(w,trigger).disabled(),Some(true)); assert!(message(w,&selected_label)); checkpoint("denied"); w.remove_window(); return; }
            Unavailable if message(w,"Destination unavailable") => { checkpoint("unavailable"); w.remove_window(); return; }
            Expired if message(w,"Session expired. Sign in again.") => { checkpoint("expired"); w.remove_window(); return; }
            Uncertain if message(w,if trash {"Delete not confirmed. Read the board before another delete; this request will not be repeated."} else {"Archive not confirmed. Read the board before another archive; this request will not be repeated."}) => { assert_eq!(hit(w,"move-confirm").disabled(),Some(true)); checkpoint("uncertain"); click(&hit(w,"move-review"),w,cx); Reconciled }
            Reconciled if message(w,if trash {"Board reloaded. Review accepted items before another delete."} else {"Board reloaded. Review accepted items before another archive."}) => { order(w,if mode.ends_with("-review-failed") {&[73,61,67,77]} else {saved_order}); checkpoint("reconciled"); w.remove_window(); return; }
            _=>step,
        };
        schedule(v,mode,next,w);
    });
}
