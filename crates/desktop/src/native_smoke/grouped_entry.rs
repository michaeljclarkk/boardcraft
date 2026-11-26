//! The fixture drives actual Kit inputs; identities are checked against mounted controls.
use super::*;
use crate::connected::Connected;
use gpui_kit::test::TestQueryExt;

const A: &str = "g-old";
const B: &str = "g-z";
const C: &str = "g-end";

fn field(group: &str) -> SharedString {
    format!("add-{group}").into()
}
fn title(id: Uuid) -> SharedString {
    format!("title-{id}").into()
}
fn input(id: Uuid) -> SharedString {
    format!("editor-{id}").into()
}

#[derive(Clone, Copy, Debug)]
enum Step {
    List,
    Open,
    Blank,
    NoBlank,
    Cancel,
    Cancelled,
    OtherDraft,
    LongDraft,
    Rejected,
    Corrected,
    CreatePending,
    Created,
    Focus(Uuid),
    CancelName(Uuid),
    EscapeName(Uuid),
    NameCancelled(Uuid),
    Unchanged(Uuid),
    NoChange(Uuid),
    NameDraft(Uuid),
    NameSubmit(Uuid),
    NamePending(Uuid),
    Renamed(Uuid),
    Revisit(Uuid),
    Revisited(Uuid),
    RecoveryDraft,
    RecoverySubmit,
    RecoveryResult,
    Review,
    NameCorrection,
    CorrectedName,
    CorrectionSaved,
    ViewerAttempt,
    ViewerChecked,
    Expired,
}

pub(crate) fn install(view: Entity<Connected>, mode: &str, w: &Window) {
    schedule(view, mode.to_owned(), Step::List, w);
}

fn roots(w: &Window, created: Option<(Uuid, &str)>, writable: bool) {
    let mut bottom = None;
    let mut left = None;
    for (group, name, count, ids) in [
        (A, "Work", "1 item", &[59][..]),
        (
            B,
            "Work",
            if created.is_some() {
                "3 items"
            } else {
                "2 items"
            },
            &[61, 63][..],
        ),
        (C, "Reserve", "0 items", &[][..]),
    ] {
        let disclosure = &w.find_all(SharedString::from(format!("fold-{group}")))[0];
        assert_eq!(disclosure.label(), Some(format!("Toggle {name}").as_str()));
        assert_eq!(disclosure.expanded(), Some(group != C));
        assert!(bottom.is_none_or(|b| b <= disclosure.bounds().top()));
        assert!(left.is_none_or(|x| x == disclosure.bounds().left()));
        left = Some(disclosure.bounds().left());
        bottom = Some(disclosure.bounds().bottom());
        assert!(message(w, count));
        for n in ids {
            let hit = &w.find_all(title(Uuid::from_u128(*n)))[0];
            assert_eq!(hit.label(), Some("Ink"));
            assert_eq!(hit.disabled() == Some(true), !writable);
            assert!(bottom.unwrap() <= hit.bounds().top());
            bottom = Some(hit.bounds().bottom());
        }
        assert_eq!(
            w.find_all(field(group)).len(),
            usize::from(writable && group != C)
        );
        if group == B
            && let Some((id, name)) = created
        {
            let hit = &w.find_all(title(id))[0];
            assert_eq!(hit.label(), Some(name));
            assert!(bottom.unwrap() <= hit.bounds().top());
            bottom = Some(hit.bounds().bottom());
        }
    }
    for n in [57, 67, 62, 73, 42, 17] {
        assert!(w.find_all(title(Uuid::from_u128(n))).is_empty());
    }
    assert!(w.find_all(field(C)).is_empty());
    assert!(
        w.find_all(SharedString::from(format!("submit-{C}")))
            .is_empty()
    );
}

fn root_ids(w: &Window) -> Vec<Uuid> {
    w.elements()
        .iter()
        .filter_map(|e| {
            let Some(ElementId::Name(name)) = e.path().last() else {
                return None;
            };
            name.strip_prefix("title-")?.parse().ok()
        })
        .collect()
}

fn new_id(w: &Window) -> Uuid {
    let roots = root_ids(w);
    assert_eq!(roots.len(), 4);
    *roots.last().unwrap()
}

fn schedule(view: Entity<Connected>, mode: String, step: Step, w: &Window) {
    w.on_next_frame(move |w, cx| {
        use self::Step::*;
        let count = view.read(cx).submissions;
        let rename = mode.ends_with("-name") || mode == "--entry-name-validation";
        let id = Uuid::from_u128(61);
        let next = match step {
            List if message(w, "Boards") => {
                click(&w.find_all(navigation::id("board", 201))[0], w, cx);
                Open
            }
            Open if message(w, "Main table") => {
                roots(w, None, mode != "--entry-viewer");
                assert_eq!(count, 0);
                if mode == "--entry-member" {
                    checkpoint("opened");
                    click(&w.find_all(field(B))[0], w, cx);
                    w.input("   ", cx);
                    Blank
                } else if mode == "--entry-viewer" {
                    w.double_click(title(id), cx);
                    ViewerAttempt
                } else if rename {
                    w.double_click(title(id), cx);
                    RecoveryDraft
                } else {
                    click(&w.find_all(field(B))[0], w, cx);
                    w.input(" Ink ", cx);
                    RecoverySubmit
                }
            }
            Blank => {
                editor(&w.find_all(field(B))[0], "   ");
                w.press("enter", cx);
                NoBlank
            }
            NoBlank => {
                assert_eq!(count, 0);
                replace("Discard this", w, cx);
                Cancel
            }
            Cancel => {
                editor(&w.find_all(field(B))[0], "Discard this");
                w.press("escape", cx);
                Cancelled
            }
            Cancelled => {
                editor(&w.find_all(field(B))[0], "");
                assert_eq!(count, 0);
                checkpoint("cancelled");
                click(&w.find_all(field(A))[0], w, cx);
                w.input(" Other draft ", cx);
                OtherDraft
            }
            OtherDraft => {
                editor(&w.find_all(field(A))[0], " Other draft ");
                click(&w.find_all(field(B))[0], w, cx);
                replace(&"😀".repeat(128), w, cx);
                LongDraft
            }
            LongDraft => {
                editor(&w.find_all(field(B))[0], &"😀".repeat(128));
                assert_eq!(count, 0); // leaving the other group's field never submits
                w.press("enter", cx);
                Rejected
            }
            Rejected if message(w, "Not saved. Correct the draft before submitting again.") => {
                roots(w, None, true);
                assert_eq!(w.find_all(field(B))[0].value(), Some("😀".repeat(128).as_str()));
                assert_eq!(w.find_all(field(A))[0].value(), Some(" Other draft "));
                assert_eq!(count, 1);
                checkpoint("rejected");
                click(&w.find_all(field(B))[0], w, cx);
                replace(" Ink ", w, cx);
                Corrected
            }
            Corrected => {
                editor(&w.find_all(field(B))[0], " Ink ");
                checkpoint("corrected");
                w.press("enter", cx);
                CreatePending
            }
            CreatePending if message(w, "Adding item…") => {
                assert_eq!(root_ids(w).len(), 3);
                assert!(message(w, "2 items"));
                assert!(!message(w, "3 items"));
                assert_eq!(w.find_all(field(B))[0].value(), Some(" Ink "));
                // Kit's text editor exposes its value, but not a disabled flag.
                // Try deleting from the focused draft while the fixture holds the write.
                w.press("backspace", cx);
                assert_eq!(w.find_all(field(B))[0].value(), Some(" Ink "));
                w.press("enter", cx);
                let add = &w.find_all(SharedString::from(format!("submit-{B}")))[0];
                assert_eq!(add.disabled(), Some(true));
                click(add, w, cx);
                assert_eq!(view.read(cx).submissions, 2);
                checkpoint("pending");
                Created
            }
            Created if message(w, "3 items") && !message(w, "Adding item…") => {
                let id = new_id(w);
                roots(w, Some((id, "Ink")), true);
                assert_eq!(w.find_all(field(B))[0].value(), Some(""));
                assert_eq!(w.find_all(field(A))[0].value(), Some(" Other draft "));
                assert_eq!(count, 2);
                checkpoint("created");
                click(&w.find_all(title(id))[0], w, cx);
                Focus(id)
            }
            Focus(id) => {
                assert_eq!(w.find_all(title(id))[0].focused(), Some(true));
                w.press("f2", cx);
                CancelName(id)
            }
            CancelName(id) => {
                editor(&w.find_all(input(id))[0], "Ink");
                replace("Discard name", w, cx);
                EscapeName(id)
            }
            EscapeName(id) => {
                editor(&w.find_all(input(id))[0], "Discard name");
                w.press("escape", cx);
                NameCancelled(id)
            }
            NameCancelled(id) => {
                roots(w, Some((id, "Ink")), true);
                assert_eq!(w.find_all(title(id))[0].focused(), Some(true));
                assert!(w.find_all(input(id)).is_empty());
                assert_eq!(count, 2);
                checkpoint("name-cancelled");
                w.press("f2", cx);
                Unchanged(id)
            }
            Unchanged(id) => {
                editor(&w.find_all(input(id))[0], "Ink");
                w.press("enter", cx);
                NoChange(id)
            }
            NoChange(id) => {
                assert_eq!(count, 2);
                checkpoint("unchanged");
                w.double_click(title(id), cx);
                NameDraft(id)
            }
            NameDraft(id) => {
                editor(&w.find_all(input(id))[0], "Ink");
                replace(" Coral ", w, cx);
                NameSubmit(id)
            }
            NameSubmit(id) => {
                editor(&w.find_all(input(id))[0], " Coral ");
                checkpoint("name-draft");
                click(&w.find_all(title(Uuid::from_u128(61)))[0], w, cx);
                NamePending(id)
            }
            NamePending(id) if message(w, "Saving…") => {
                assert_eq!(w.find_all(title(id))[0].label(), Some(" Coral "));
                assert!(w.find_all(input(id)).is_empty());
                assert_eq!(count, 3);
                checkpoint("name-pending");
                Renamed(id)
            }
            Renamed(id) if !message(w, "Saving name…") => {
                roots(w, Some((id, "Coral")), true);
                assert_eq!(w.find_all(title(Uuid::from_u128(61)))[0].focused(), Some(true));
                assert_eq!(w.find_all(field(A))[0].value(), Some(" Other draft "));
                assert_eq!(count, 3);
                checkpoint("renamed");
                click(&w.find_all("board-list")[0], w, cx);
                Revisit(id)
            }
            Revisit(id) if message(w, "Boards") => {
                click(&w.find_all(navigation::id("board", 201))[0], w, cx);
                Revisited(id)
            }
            Revisited(id) if message(w, "Main table") => {
                roots(w, Some((id, "Coral")), true);
                assert_eq!(count, 3);
                assert_eq!(new_id(w), id);
                checkpoint("revisited");
                w.remove_window();
                return;
            }
            RecoveryDraft => {
                editor(&w.find_all(input(id))[0], "Ink");
                replace(if mode == "--entry-name-validation" { "   " } else { " Coral " }, w, cx);
                RecoverySubmit
            }
            RecoverySubmit => {
                editor(&w.find_all(if rename { input(id) } else { field(B) })[0], if rename {
                    if mode == "--entry-name-validation" { "   " } else { " Coral " }
                } else { " Ink " });
                checkpoint("draft");
                w.press("enter", cx);
                if mode == "--entry-revoked-name" { Expired } else { RecoveryResult }
            }
            RecoveryResult if mode == "--entry-name-validation" && message(w, "Not saved. Correct the draft before submitting again.") => {
                roots(w, None, true);
                assert_eq!(count, 1);
                assert!(w.find_all(input(id)).is_empty());
                checkpoint("name-rejected");
                click(&w.find_all(SharedString::from(format!("correct-{id}")))[0], w, cx);
                NameCorrection
            }
            NameCorrection => {
                editor(&w.find_all(input(id))[0], "   ");
                assert_eq!(count, 1);
                replace(" Coral ", w, cx);
                CorrectedName
            }
            CorrectedName => {
                editor(&w.find_all(input(id))[0], " Coral ");
                checkpoint("name-corrected");
                w.press("enter", cx);
                CorrectionSaved
            }
            CorrectionSaved if !message(w, "Saving name…") && w.find_all(title(id)).first().is_some_and(|h| h.label() == Some("Coral")) => {
                assert_eq!(count, 2);
                checkpoint("name-corrected-saved");
                w.remove_window();
                return;
            }
            RecoveryResult if mode.starts_with("--entry-demoted") && message(w, "Board items · Read only") && message(w, "Not saved: permission denied.") => {
                roots(w, None, false);
                assert_eq!(count, 1);
                checkpoint("denied");
                w.double_click(title(id), cx);
                ViewerAttempt
            }
            RecoveryResult if message(w, "Save not confirmed. Reload board to check accepted items; the write will not be repeated.") => {
                roots(w, None, false);
                assert_eq!(count, 1);
                checkpoint("unconfirmed");
                click(&w.find_all("reload-table")[0], w, cx);
                Review
            }
            Review if message(w, "Board reloaded. Review accepted items before making another change.") => {
                if rename {
                    assert_eq!(w.find_all(title(id))[0].label(), Some("Coral"));
                    assert_eq!(w.find_all(title(Uuid::from_u128(63)))[0].label(), Some("Ink"));
                } else {
                    roots(w, Some((new_id(w), "Ink")), true);
                    assert_eq!(w.find_all(field(B))[0].value(), Some(""));
                }
                assert_eq!(count, 1);
                checkpoint("reconciled");
                w.remove_window();
                return;
            }
            ViewerAttempt => {
                assert!(w.find_all(input(id)).is_empty());
                w.press("f2", cx);
                ViewerChecked
            }
            ViewerChecked => {
                assert!(w.find_all(input(id)).is_empty());
                assert_eq!(count, usize::from(mode != "--entry-viewer"));
                checkpoint("viewer");
                w.remove_window();
                return;
            }
            Expired if !w.find_all("identifier").is_empty() => {
                assert!(w.find_all(title(id)).is_empty());
                assert!(w.find_all(field(B)).is_empty());
                assert_eq!(count, 1);
                checkpoint("expired");
                w.remove_window();
                return;
            }
            _ => step,
        };
        schedule(view, mode, next, w);
    });
}
