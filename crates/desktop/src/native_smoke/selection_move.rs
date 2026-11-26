//! Native pointer, Space, Shift and dialog inputs with parent-owned database checkpoints.
use super::*;
use crate::connected::Connected;
use gpui_kit::test::TestQueryExt;

#[derive(Clone, Copy, Debug)]
enum Step {
    List,
    Open,
    Draft,
    Hidden,
    Range,
    Menu,
    HeadingSelected,
    Cleared,
    RangeAgain,
    Expanded,
    First,
    Keyboard(usize, u8),
    Second,
    Third,
    Selected,
    Choose,
    Cancel,
    Cancelled(Uuid),
    Reopened(Uuid),
    Pending,
    Saved,
    GroupFirst,
    GroupSecond,
    GroupAll,
    GroupDialog,
    GroupCancel,
    Reloaded,
    Refused,
    Reviewed,
    Adopted,
    ConfirmKeyboard(usize),
    Deliberate,
    Denied,
    Viewer,
    Uncertain,
    Reconciled,
    DraftRestored,
    RangeFailure,
    RangeRetry,
    RangeCleared,
}
fn id(prefix: &str, n: u128) -> SharedString {
    format!("{prefix}-{}", Uuid::from_u128(n)).into()
}
fn hit(w: &Window, name: &str) -> ElementSnapshot {
    w.find_all(SharedString::from(name.to_owned()))[0].clone()
}
fn row(w: &Window, n: u128) -> ElementSnapshot {
    w.find_all(id("select", n))[0].clone()
}
fn checked(w: &Window, expected: &[u128]) {
    for n in [73, 61, 67, 77, 59] {
        if let Some(r) = w.find_all(id("select", n)).first() {
            assert_eq!(r.role(), Some(Role::CheckBox));
            assert_eq!(r.checked(), Some(expected.contains(&n)));
            assert_eq!(
                r.label(),
                Some(if n == 73 {
                    "Select Sketch"
                } else if n == 77 {
                    "Select Fold"
                } else if n == 61 && r.label() == Some("Select Indigo") {
                    "Select Indigo"
                } else {
                    "Select Ink"
                })
            );
        }
        assert!(w.find_all(id("editor", n)).is_empty());
    }
    assert_eq!(
        !w.find_all("selection-clear").is_empty(),
        !expected.is_empty()
    );
    if !expected.is_empty() {
        assert!(message(w, &format!("{} selected", expected.len())));
    }
    for n in [57, 62, 42, 17] {
        assert!(w.find_all(id("select", n)).is_empty());
    }
}
fn order(w: &Window, expected: &[u128]) {
    let actual: Vec<_> = w
        .elements()
        .iter()
        .filter_map(|e| {
            let Some(ElementId::Name(name)) = e.path().last() else {
                return None;
            };
            Some(name.strip_prefix("title-")?.parse::<Uuid>().ok()?.as_u128())
        })
        .collect();
    assert_eq!(actual, expected);
}
pub(crate) fn install(v: Entity<Connected>, mode: &str, w: &Window) {
    schedule(v, mode.to_owned(), Step::List, w);
}
fn schedule(v: Entity<Connected>, mode: String, step: Step, w: &Window) {
    w.on_next_frame(move |w,cx| {
        use self::Step::*;
        let happy=mode=="--selection-move";
        let count=v.read(cx).submissions;
        let next=match step {
            List if message(w,"Boards") => { click(&w.find_all(id("board",201))[0],w,cx); Open }
            Open if message(w,"Main table") => {
                checked(w,&[]); order(w,&[73,61,67,77,59]);
                assert_eq!(hit(w,"fold-g-10").expanded(),Some(false));
                assert_eq!(hit(w,"select-group-g-90").indeterminate(),Some(false));
                checkpoint("opened");
                if happy { click(&hit(w,"add-g-20"),w,cx); w.input(" Other draft ",cx); Draft }
                else {
                    if mode=="--selection-demoted" { click(&hit(w,"add-g-20"),w,cx); w.input(" Other draft ",cx); }
                    click(&row(w,67),w,cx); First
                }
            }
            Draft => {
                editor(&hit(w,"add-g-20")," Other draft ");
                click(&row(w,59),w,cx);
                click(&hit(w,"fold-g-20"),w,cx); Hidden
            }
            Hidden if hit(w,"fold-g-20").expanded()==Some(false) && !message(w,"Saving group presentation…") => {
                assert!(w.find_all(id("select",59)).is_empty()); checked(w,&[59]); checkpoint("hidden");
                w.change_modifiers(Modifiers { shift:true,..Default::default() },cx);
                click(&row(w,67),w,cx); w.change_modifiers(Modifiers::default(),cx); Range
            }
            Range if message(w,"3 selected") => {
                checked(w,&[67,77,59]); checkpoint("hidden-range");
                click(&hit(w,"group-menu-g-20"),w,cx); Menu
            }
            Menu if w.try_find_by_label("Select all items in group").is_some() => {
                click(&w.find_by_label("Select all items in group"),w,cx); HeadingSelected
            }
            HeadingSelected if w.try_find_by_label("Select all items in group").is_none() => {
                checked(w,&[67,77,59]); checkpoint("heading-selected");
                click(&hit(w,"selection-clear"),w,cx); Cleared
            }
            Cleared => {
                checked(w,&[]);
                w.change_modifiers(Modifiers { shift:true,..Default::default() },cx);
                click(&row(w,67),w,cx); w.change_modifiers(Modifiers::default(),cx); RangeAgain
            }
            RangeAgain if message(w,"3 selected") => {
                checked(w,&[67,77,59]);
                click(&hit(w,"fold-g-20"),w,cx); Expanded
            }
            Expanded if hit(w,"fold-g-20").expanded()==Some(true) && !message(w,"Saving group presentation…") => {
                checked(w,&[67,77,59]); assert_eq!(hit(w,"add-g-20").value(),Some(" Other draft "));
                checkpoint("expanded"); click(&hit(w,"selection-clear"),w,cx); click(&row(w,67),w,cx); First
            }
            First => {
                checked(w,&[67]);
                if mode=="--selection-viewer-range" {
                    checkpoint("range-draft");
                    w.change_modifiers(Modifiers {shift:true,..Default::default()},cx); click(&row(w,59),w,cx); w.change_modifiers(Modifiers::default(),cx);
                    RangeFailure
                } else if happy { Keyboard(0,0) } else { click(&row(w,59),w,cx); Second }
            }
            RangeFailure if message(w,"Could not read selection. Repeat the gesture to retry, or reload board.") => {
                checked(w,&[67]); checkpoint("range-refused");
                w.change_modifiers(Modifiers {shift:true,..Default::default()},cx); click(&row(w,59),w,cx); w.change_modifiers(Modifiers::default(),cx);
                RangeRetry
            }
            RangeRetry if message(w,"3 selected") => {
                checked(w,&[67,77,59]); checkpoint("range-retried");
                w.change_modifiers(Modifiers {shift:true,..Default::default()},cx); click(&row(w,59),w,cx); w.change_modifiers(Modifiers::default(),cx);
                RangeCleared
            }
            RangeCleared if w.find_all("selection-clear").is_empty() => {
                checked(w,&[]); assert_eq!(count,0); checkpoint("range-cleared"); w.remove_window(); return;
            }
            Keyboard(tabs,round) => {
                if row(w,67).focused()==Some(true) {
                    w.press("space",cx);
                    if round==0 { Keyboard(tabs,1) } else { checked(w,&[67]); click(&row(w,59),w,cx); Second }
                } else { assert!(tabs<40); w.press("tab",cx); Keyboard(tabs+1,round) }
            }
            Second => { checked(w,&[67,59]); click(&row(w,61),w,cx); Third }
            Third => {
                checked(w,&[67,59,61]);
                if mode=="--selection-viewer" { assert_eq!(hit(w,"selection-move").disabled(),Some(true)); checkpoint("viewer"); click(&hit(w,"selection-clear"),w,cx); Viewer }
                else { click(&hit(w,"selection-move"),w,cx); Selected }
            }
            Selected if !w.find_all("move-confirm").is_empty() => {
                assert_ne!(hit(w,"move-confirm").disabled(),Some(true));
                if happy { click(&w.find_by_label("Destination group"),w,cx); Choose }
                else { checkpoint("draft"); click(&hit(w,"move-confirm"),w,cx);
                    if mode=="--selection-conflict" || mode=="--selection-missing" { Refused } else if mode=="--selection-demoted" || mode=="--selection-denied" { Denied } else { Uncertain } }
            }
            Choose => {
                assert_eq!(w.find_by_label("Destination group").expanded(),Some(true));
                w.press("down",cx); w.press("down",cx);
                w.press("enter",cx); Cancel
            }
            Cancel => {
                let (op,group,ids)=v.read(cx).selection_move_snapshot(cx).unwrap();
                assert_eq!(group.as_ref(),"g-20");
                assert_eq!(ids,[67,59,61].map(Uuid::from_u128));
                assert_eq!(count,2);
                checkpoint("cancel-draft"); w.press("escape",cx); Cancelled(op)
            }
            Cancelled(op) if w.find_all("move-confirm").is_empty() => {
                checked(w,&[67,59,61]); assert_eq!(hit(w,"selection-move").focused(),Some(true));
                checkpoint("cancelled"); click(&hit(w,"selection-move"),w,cx); Reopened(op)
            }
            Reopened(op) if !w.find_all("move-confirm").is_empty() => {
                let (new,group,ids)=v.read(cx).selection_move_snapshot(cx).unwrap();
                assert_ne!(new,op); assert_eq!(group.as_ref(),"g-90"); assert_eq!(ids,[67,59,61].map(Uuid::from_u128));
                checkpoint("confirming"); click(&hit(w,"move-confirm"),w,cx); Pending
            }
            Pending if count==3 && hit(w,"move-confirm").disabled()==Some(true) => {
                order(w,&[73,61,67,77,59]); checked(w,&[67,59,61]);
                assert_eq!(hit(w,"move-confirm").disabled(),Some(true));
                click(&hit(w,"move-confirm"),w,cx); assert_eq!(v.read(cx).submissions,3);
                checkpoint("pending"); Saved
            }
            Saved if w.find_all("move-confirm").is_empty() && !message(w,"Moving selected items…") => {
                order(w,&[73,77,67,59,61]); checked(w,&[]);
                assert_eq!(hit(w,"add-g-20").value(),Some(" Other draft "));
                assert_eq!(hit(w,"fold-g-10").expanded(),Some(false));
                assert_eq!(hit(w,"fold-g-90").focused(),Some(true));
                checkpoint("saved"); click(&row(w,67),w,cx); GroupFirst
            }
            GroupFirst => { click(&row(w,73),w,cx); GroupSecond }
            GroupSecond => {
                assert_eq!(hit(w,"select-group-g-90").indeterminate(),Some(true));
                click(&hit(w,"select-group-g-90"),w,cx); GroupAll
            }
            GroupAll if message(w,"5 selected") => {
                checked(w,&[73,77,67,59,61]); assert_eq!(hit(w,"select-group-g-90").checked(),Some(true));
                click(&hit(w,"selection-move"),w,cx); GroupDialog
            }
            GroupDialog if !w.find_all("move-confirm").is_empty() => {
                assert_eq!(v.read(cx).selection_move_snapshot(cx).unwrap().2,[73,77,67,59,61].map(Uuid::from_u128));
                checkpoint("group-order"); click(&hit(w,"move-cancel"),w,cx); GroupCancel
            }
            GroupCancel if w.find_all("move-confirm").is_empty() => { click(&hit(w,"reload-table"),w,cx); Reloaded }
            Reloaded if message(w,"Board reloaded. Review accepted items before making another change.") => {
                checked(w,&[]); order(w,&[73,77,67,59,61]); assert_eq!(count,3); checkpoint("reloaded"); w.remove_window(); return;
            }
            Refused if message(w,"Items changed or are unavailable. Review latest items before confirming again.") => {
                checked(w,&[67,59,61]); assert_eq!(hit(w,"move-confirm").disabled(),Some(true)); checkpoint("conflict");
                click(&hit(w,"move-review"),w,cx); Reviewed
            }
            Reviewed if !w.find_all("move-adopt").is_empty() => {
                if mode=="--selection-missing" {
                    assert!(message(w,&format!("{} · unavailable (requested revision 1)",Uuid::from_u128(61))));
                    assert_eq!(hit(w,"move-adopt").disabled(),Some(true));
                    assert_eq!(hit(w,"move-confirm").disabled(),Some(true));
                    checkpoint("missing-reviewed");
                    w.remove_window(); return;
                }
                assert!(message(w,&format!("Indigo · {} · revision 2 (requested 1)",Uuid::from_u128(61))));
                assert_ne!(hit(w,"move-adopt").disabled(),Some(true));
                checkpoint("reviewed"); click(&hit(w,"move-adopt"),w,cx); Adopted
            }
            Adopted if message(w,"Revisions adopted. Confirm separately to move the selected items.") => {
                assert_eq!(count,1); checkpoint("adopted"); ConfirmKeyboard(0)
            }
            ConfirmKeyboard(tabs) => {
                if hit(w,"move-confirm").focused()==Some(true) { w.press("enter",cx); Deliberate }
                else { assert!(tabs<12); w.press("tab",cx); ConfirmKeyboard(tabs+1) }
            }
            Deliberate if w.find_all("move-confirm").is_empty() => {
                checked(w,&[]); order(w,&[73,77,67,59,61]); assert_eq!(count,2);
                assert_eq!(w.find_all(id("title",61))[0].label(),Some("Indigo"));
                checkpoint("deliberate"); w.remove_window(); return;
            }
            Denied if message(w,"Move not saved: permission denied.") || message(w,"Destination unavailable") => {
                assert_eq!(count,1);
                if mode=="--selection-demoted" {
                    checked(w,&[67,59,61]); assert_eq!(hit(w,"selection-move").disabled(),Some(true));
                    assert_eq!(hit(w,"fold-g-90").focused(),Some(true));
                    assert_eq!(hit(w,"fold-g-10").expanded(),Some(false));
                    checkpoint("denied"); click(&hit(w,"selection-clear"),w,cx); Viewer
                } else { checkpoint("unavailable"); w.remove_window(); return; }
            }
            Viewer => {
                checked(w,&[]); checkpoint("cleared");
                if mode=="--selection-demoted" { click(&hit(w,"reload-table"),w,cx); DraftRestored }
                else { w.remove_window(); return; }
            }
            DraftRestored if message(w,"Board reloaded. Review accepted items before making another change.") => {
                assert_eq!(hit(w,"add-g-20").value(),Some(" Other draft "));
                assert_eq!(count,1); checked(w,&[]); checkpoint("draft-restored"); w.remove_window(); return;
            }
            Uncertain if message(w,"Move not confirmed. Read the board before another move; this request will not be repeated.") => {
                assert_eq!(hit(w,"move-confirm").disabled(),Some(true)); checked(w,&[67,59,61]);
                checkpoint("uncertain"); click(&hit(w,"move-review"),w,cx); Reconciled
            }
            Reconciled if message(w,"Board reloaded. Review accepted items before another move.") => {
                checked(w,&[]); order(w,&[73,77,67,59,61]); assert_eq!(count,1); checkpoint("reconciled"); w.remove_window(); return;
            }
            _=>step,
        };
        schedule(v,mode,next,w);
    });
}
