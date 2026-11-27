//! The parent freezes each checkpoint for fresh-connection, whole-record checks.
use super::*;
use crate::connected::Connected;
use gpui_kit::test::TestQueryExt;

#[derive(Clone, Copy, Debug)]
enum Step {
    List,
    Open,
    Prepared,
    NewMenu,
    Draft,
    Caret,
    Blank,
    Cancelled,
    ReopenMenu,
    Reopened,
    Colour,
    Chosen,
    Pending,
    Created,
    Edit,
    EditMenu,
    Unchanged,
    Edited,
    RenameMenu,
    Rename,
    EditPending,
    EditColour,
    EditChosen,
    Saved,
    NameOnly,
    Named,
    ColourMenu,
    ColourOnly,
    ColourOnlyChosen,
    Coloured,
    Revisit,
    Revisiting,
    Revisited,
    Refused,
    Conflict,
    Reviewed,
    Adopted,
    Deliberate,
    Uncertain,
    Reconciled,
    Denied,
    DisabledMenu,
    DisabledCreateMenu,
    DisabledDone,
    ColourCancelMenu,
    ColourCancelOpened,
    ColourCancelSelected,
    ColourCancelled,
    CancelReturned,
    ReadRefused,
    CreatedEditor,
    CreatedSelect,
    CreatedSubmit,
    CreatedSaved,
}
fn hit(w: &Window, id: &str) -> ElementSnapshot {
    w.find_all(SharedString::from(id.to_owned()))
        .first()
        .unwrap_or_else(|| panic!("missing control {id}"))
        .clone()
}
fn title(n: u128) -> SharedString {
    format!("title-{}", Uuid::from_u128(n)).into()
}
fn input(w: &Window, value: &str, focused: bool) {
    let name = hit(w, "group-field-name");
    assert_eq!(name.value(), Some(value));
    if focused {
        assert_eq!(name.focused(), Some(true));
    }
}
fn retained(w: &Window) {
    assert_eq!(hit(w, "fold-g-old").expanded(), Some(false));
    if let Some(entry) = w.find_all("add-g-end").first() {
        assert_eq!(entry.value(), Some(" Other draft "));
    }
    assert_eq!(
        w.find_all(SharedString::from(format!(
            "select-{}",
            Uuid::from_u128(61)
        )))[0]
            .checked(),
        Some(true)
    );
    assert_eq!(w.find_all(title(61))[0].label(), Some("Ink"));
    assert_eq!(w.find_all(title(63))[0].label(), Some("Ink"));
    assert!(w.find_all(title(59)).is_empty());
    assert!(w.find_all(title(67)).is_empty());
    assert!(w.find_all(title(57)).is_empty());
    assert!(w.find_all(title(62)).is_empty());
}
fn headings(w: &Window) -> Vec<String> {
    w.elements()
        .iter()
        .filter_map(|e| {
            let Some(ElementId::Name(name)) = e.path().last() else {
                return None;
            };
            name.strip_prefix("group-name-").map(str::to_owned)
        })
        .collect()
}
pub(crate) fn install(v: Entity<Connected>, mode: &str, w: &Window) {
    schedule(v, mode.to_owned(), Step::List, w);
}
fn schedule(v: Entity<Connected>, mode: String, step: Step, w: &Window) {
    w.on_next_frame(move |w, cx| {
        use self::Step::*;
        let happy = mode == "--groups-fields";
        let create = happy || mode.contains("create");
        let conflict = mode.contains("conflict");
        let count = v.read(cx).submissions;
        let next = match step {
            List if message(w, "Boards") => {
                click(
                    &w.find_all(SharedString::from(format!(
                        "board-{}",
                        Uuid::from_u128(201)
                    )))[0],
                    w,
                    cx,
                );
                Open
            }
            Open if message(w, "Main table") => {
                assert_eq!(hit(w, "group-name-g-z").label(), Some("Work"));
                assert_eq!(hit(w, "group-name-g-old").label(), Some("Work"));
                checkpoint("opened");
                if mode.contains("member") || mode.contains("viewer") {
                    assert_eq!(hit(w, "group-name-g-z").disabled(), Some(true));
                    click(&hit(w, "group-menu-g-z"), w, cx);
                    DisabledMenu
                } else {
                    click(&hit(w, "add-g-end"), w, cx);
                    w.input(" Other draft ", cx);
                    click(
                        &w.find_all(SharedString::from(format!(
                            "select-{}",
                            Uuid::from_u128(61)
                        )))[0],
                        w,
                        cx,
                    );
                    Prepared
                }
            }
            DisabledMenu if w.try_find_by_label("Rename group…").is_some() => {
                click(&w.find_by_label("Rename group…"), w, cx);
                click(&w.find_by_label("Change group colour…"), w, cx);
                assert!(w.find_all("group-field-name").is_empty());
                w.press("escape", cx);
                DisabledCreateMenu
            }
            DisabledCreateMenu if w.try_find_by_label("Rename group…").is_none() => {
                click(&hit(w, "new-item-options"), w, cx);
                DisabledDone
            }
            DisabledDone if w.try_find_by_label("New group of items…").is_some() => {
                click(&w.find_by_label("New group of items…"), w, cx);
                assert!(w.find_all("group-field-name").is_empty());
                assert_eq!(count, 0);
                checkpoint("disabled");
                w.remove_window();
                return;
            }
            Prepared => {
                retained(w);
                // A single click is not an editor.
                click(&hit(w, "group-name-g-z"), w, cx);
                assert!(w.find_all("group-field-name").is_empty());
                if create {
                    click(&hit(w, "new-item-options"), w, cx);
                    NewMenu
                } else {
                    w.double_click("group-name-g-z", cx);
                    Reopened
                }
            }
            NewMenu if w.try_find_by_label("New group of items…").is_some() => {
                click(&w.find_by_label("New group of items…"), w, cx);
                Draft
            }
            Draft if !w.find_all("group-field-name").is_empty() => {
                input(w, "New group", true);
                assert_eq!(w.find_by_label("Group colour").value(), Some("#579bfc"));
                w.input("!", cx);
                Caret
            }
            Caret => {
                input(w, "New group!", true);
                replace("   ", w, cx);
                Blank
            }
            Blank => {
                input(w, "   ", true);
                assert_eq!(hit(w, "group-field-save").disabled(), Some(true));
                checkpoint("cancel-ready");
                w.press("escape", cx);
                Cancelled
            }
            Cancelled if w.find_all("group-field-name").is_empty() => {
                retained(w);
                assert_eq!(count, 0);
                assert_eq!(hit(w, "new-item-options").focused(), Some(true));
                checkpoint("cancelled");
                click(&hit(w, "new-item-options"), w, cx);
                ReopenMenu
            }
            ReopenMenu if w.try_find_by_label("New group of items…").is_some() => {
                click(&w.find_by_label("New group of items…"), w, cx);
                Reopened
            }
            Reopened if !w.find_all("group-field-name").is_empty() => {
                input(w, if create { "New group" } else { "Work" }, true);
                assert_eq!(
                    w.find_by_label("Group colour").value(),
                    Some(if create { "#579bfc" } else { "#443322" })
                );
                replace("  Work  ", w, cx);
                click(&w.find_by_label("Group colour"), w, cx);
                Colour
            }
            Colour if w.find_by_label("Group colour").expanded() == Some(true) => {
                w.press(if create { "down" } else { "up" }, cx);
                w.press("enter", cx);
                Chosen
            }
            Chosen if w.find_by_label("Group colour").expanded() != Some(true) => {
                input(w, "  Work  ", false);
                assert_eq!(w.find_by_label("Group colour").value(), Some("#00c875"));
                assert_eq!(count, 0);
                checkpoint("draft");
                click(&hit(w, "group-field-save"), w, cx);
                if happy {
                    Pending
                } else if conflict {
                    Refused
                } else if mode.contains("missing") {
                    Conflict
                } else if mode.contains("demoted")
                    || mode.contains("denied")
                    || mode.contains("session")
                {
                    Denied
                } else {
                    Uncertain
                }
            }
            Pending if hit(w, "group-field-save").disabled() == Some(true) && count == 1 => {
                retained(w);
                input(w, "  Work  ", false);
                assert_eq!(hit(w, "group-field-cancel").disabled(), Some(true));
                click(&hit(w, "group-field-save"), w, cx);
                w.press("escape", cx);
                assert_eq!(v.read(cx).submissions, 1);
                checkpoint("pending");
                Created
            }
            Created if w.find_all("group-field-name").is_empty() => {
                retained(w);
                assert_eq!(count, 1);
                let ids = headings(w);
                assert_eq!(ids.len(), 4);
                assert!(ids[0].starts_with("g-"));
                assert_eq!(&ids[1..], ["g-old", "g-z", "g-end"]);
                assert_eq!(
                    hit(w, &format!("group-name-{}", ids[0])).label(),
                    Some("Work")
                );
                checkpoint("created");
                w.double_click("group-name-g-z", cx);
                Edit
            }
            Edit if !w.find_all("group-field-name").is_empty() => {
                input(w, "Work", true);
                assert_eq!(w.find_by_label("Group colour").value(), Some("#443322"));
                replace("  ", w, cx);
                EditMenu
            }
            EditMenu => {
                assert_eq!(hit(w, "group-field-save").disabled(), Some(true));
                click(&w.find_by_label("Group colour"), w, cx);
                w.press("escape", cx);
                w.press("escape", cx);
                Unchanged
            }
            Unchanged if w.find_all("group-field-name").is_empty() => {
                retained(w);
                assert_eq!(count, 1);
                checkpoint("edit-cancelled");
                click(&hit(w, "group-menu-g-z"), w, cx);
                RenameMenu
            }
            RenameMenu if w.try_find_by_label("Rename group…").is_some() => {
                click(&w.find_by_label("Rename group…"), w, cx);
                Rename
            }
            Rename if !w.find_all("group-field-name").is_empty() => {
                input(w, "Work", true);
                assert_eq!(w.find_by_label("Group colour").value(), Some("#443322"));
                checkpoint("edit-confirming");
                click(&hit(w, "group-field-save"), w, cx);
                EditPending
            }
            EditPending if count == 2 && hit(w, "group-field-save").disabled() == Some(true) => {
                retained(w);
                assert_eq!(hit(w, "group-name-g-z").label(), Some("Work"));
                assert_eq!(hit(w, "group-field-cancel").disabled(), Some(true));
                click(&hit(w, "group-field-save"), w, cx);
                w.press("escape", cx);
                assert_eq!(v.read(cx).submissions, 2);
                checkpoint("edit-pending");
                Edited
            }
            Edited if w.find_all("group-field-name").is_empty() => {
                retained(w);
                assert_eq!(count, 2);
                checkpoint("unchanged");
                click(&hit(w, "group-menu-g-z"), w, cx);
                ColourCancelMenu
            }
            ColourCancelMenu if w.try_find_by_label("Change group colour…").is_some() => {
                click(&w.find_by_label("Change group colour…"), w, cx);
                ColourCancelOpened
            }
            ColourCancelOpened if !w.find_all("group-field-name").is_empty() => {
                input(w, "Work", true);
                replace("Discard this", w, cx);
                click(&w.find_by_label("Group colour"), w, cx);
                ColourCancelSelected
            }
            ColourCancelSelected if w.find_by_label("Group colour").expanded() == Some(true) => {
                w.press("up", cx);
                w.press("enter", cx);
                ColourCancelled
            }
            ColourCancelled if w.find_by_label("Group colour").expanded() != Some(true) => {
                assert_eq!(w.find_by_label("Group colour").value(), Some("#00c875"));
                click(&hit(w, "group-field-cancel"), w, cx);
                CancelReturned
            }
            CancelReturned if w.find_all("group-field-name").is_empty() => {
                retained(w);
                assert_eq!(count, 2);
                assert_eq!(hit(w, "group-name-g-z").focused(), Some(true));
                checkpoint("colour-cancelled");
                click(&hit(w, "group-menu-g-z"), w, cx);
                ColourMenu
            }
            ColourMenu if w.try_find_by_label("Change group colour…").is_some() => {
                click(&w.find_by_label("Change group colour…"), w, cx);
                EditColour
            }
            EditColour if !w.find_all("group-field-name").is_empty() => {
                input(w, "Work", true);
                replace("  Reserve  ", w, cx);
                click(&w.find_by_label("Group colour"), w, cx);
                EditChosen
            }
            EditChosen if w.find_by_label("Group colour").expanded() == Some(true) => {
                w.press("up", cx);
                w.press("enter", cx);
                Saved
            }
            Saved if w.find_by_label("Group colour").expanded() != Some(true) => {
                assert_eq!(w.find_by_label("Group colour").value(), Some("#00c875"));
                click(&hit(w, "group-field-save"), w, cx);
                NameOnly
            }
            NameOnly if w.find_all("group-field-name").is_empty() => {
                retained(w);
                assert_eq!(hit(w, "group-name-g-z").label(), Some("Reserve"));
                assert_eq!(count, 3);
                checkpoint("edited");
                w.double_click("group-name-g-z", cx);
                Named
            }
            Named if !w.find_all("group-field-name").is_empty() => {
                input(w, "Reserve", true);
                assert_eq!(w.find_by_label("Group colour").value(), Some("#00c875"));
                replace("  Ink  ", w, cx);
                w.press("enter", cx);
                ColourOnly
            }
            ColourOnly if w.find_all("group-field-name").is_empty() => {
                retained(w);
                assert_eq!(hit(w, "group-name-g-z").label(), Some("Ink"));
                assert_eq!(count, 4);
                checkpoint("name-only");
                w.double_click("group-name-g-z", cx);
                ColourOnlyChosen
            }
            ColourOnlyChosen if !w.find_all("group-field-name").is_empty() => {
                input(w, "Ink", true);
                click(&w.find_by_label("Group colour"), w, cx);
                Coloured
            }
            Coloured if w.find_by_label("Group colour").expanded() == Some(true) => {
                w.press("up", cx);
                w.press("enter", cx);
                Revisit
            }
            Revisit if w.find_by_label("Group colour").expanded() != Some(true) => {
                click(&hit(w, "group-field-save"), w, cx);
                Revisiting
            }
            Revisiting if w.find_all("group-field-name").is_empty() => {
                retained(w);
                assert_eq!(count, 5);
                checkpoint("colour-only");
                let id = headings(w)[0].clone();
                w.double_click(SharedString::from(format!("group-name-{id}")), cx);
                CreatedEditor
            }
            CreatedEditor if !w.find_all("group-field-name").is_empty() => {
                input(w, "Work", true);
                assert_eq!(w.find_by_label("Group colour").value(), Some("#00c875"));
                replace("  Reserve  ", w, cx);
                click(&w.find_by_label("Group colour"), w, cx);
                CreatedSelect
            }
            CreatedSelect if w.find_by_label("Group colour").expanded() == Some(true) => {
                w.press("up", cx);
                w.press("enter", cx);
                CreatedSubmit
            }
            CreatedSubmit if w.find_by_label("Group colour").expanded() != Some(true) => {
                click(&hit(w, "group-field-save"), w, cx);
                CreatedSaved
            }
            CreatedSaved if w.find_all("group-field-name").is_empty() => {
                retained(w);
                assert_eq!(count, 6);
                assert_eq!(
                    hit(w, &format!("group-name-{}", headings(w)[0])).label(),
                    Some("Reserve")
                );
                checkpoint("created-edited");
                click(&hit(w, "board-list"), w, cx);
                Revisited
            }
            Revisited if message(w, "Boards") => {
                click(
                    &w.find_all(SharedString::from(format!(
                        "board-{}",
                        Uuid::from_u128(201)
                    )))[0],
                    w,
                    cx,
                );
                Reconciled
            }
            Reconciled if message(w, "Main table") && w.find_all("group-field-name").is_empty() => {
                assert_eq!(
                    count,
                    if happy {
                        6
                    } else if conflict {
                        3
                    } else {
                        1
                    }
                );
                assert_eq!(hit(w, "fold-g-old").expanded(), Some(false));
                assert_eq!(
                    hit(w, "group-name-g-z").label(),
                    Some(if happy { "Ink" } else { "Work" })
                );
                if !happy {
                    retained(w);
                    assert_eq!(hit(w, "add-g-end").value(), Some(" Other draft "));
                } else {
                    assert_eq!(
                        hit(w, &format!("group-name-{}", headings(w)[0])).label(),
                        Some("Reserve")
                    );
                }
                checkpoint(if happy { "revisited" } else { "reconciled" });
                w.remove_window();
                return;
            }
            Refused if message(w, "Could not save group. Check the draft and save again.") => {
                retained(w);
                input(w, "  Work  ", false);
                assert_eq!(w.find_by_label("Group colour").value(), Some("#00c875"));
                checkpoint("refused");
                click(&hit(w, "group-field-save"), w, cx);
                Conflict
            }
            Conflict if message(w, "Board changed. Review latest board before saving again.") => {
                retained(w);
                input(w, "  Work  ", false);
                assert_eq!(hit(w, "group-field-save").disabled(), Some(true));
                checkpoint("conflict");
                click(&hit(w, "group-field-review"), w, cx);
                Reviewed
            }
            Reviewed if !w.find_all("group-field-adopt").is_empty() => {
                input(w, "  Work  ", false);
                if mode.contains("missing") {
                    assert!(message(w, "Group g-z is no longer available."));
                    assert_eq!(hit(w, "group-field-adopt").disabled(), Some(true));
                    checkpoint("missing");
                    w.remove_window();
                    return;
                }
                if !create {
                    assert!(message(
                        w,
                        "g-z · Peer / #abcdef · revision 2 (requested 1)"
                    ));
                }
                checkpoint("reviewed");
                click(&hit(w, "group-field-adopt"), w, cx);
                Adopted
            }
            Adopted
                if message(
                    w,
                    "Revision adopted. Save group separately to apply the draft.",
                ) =>
            {
                input(w, "  Work  ", true);
                assert_eq!(count, if mode.contains("absent") { 1 } else { 2 });
                checkpoint("adopted");
                w.press("enter", cx);
                Deliberate
            }
            Deliberate if w.find_all("group-field-name").is_empty() => {
                retained(w);
                assert_eq!(count, if mode.contains("absent") { 2 } else { 3 });
                checkpoint("deliberate");
                w.remove_window();
                return;
            }
            Uncertain
                if message(
                    w,
                    "Group save not confirmed. Reload board; this request will not be repeated.",
                ) =>
            {
                retained(w);
                assert_eq!(hit(w, "group-field-save").disabled(), Some(true));
                assert_eq!(hit(w, "group-field-cancel").disabled(), Some(true));
                w.press("escape", cx);
                checkpoint("uncertain");
                click(&hit(w, "group-field-review"), w, cx);
                ReadRefused
            }
            ReadRefused
                if message(
                    w,
                    "Could not read latest board. Reload board; this request will not be repeated.",
                ) =>
            {
                assert_eq!(count, 1);
                assert_eq!(hit(w, "group-field-save").disabled(), Some(true));
                assert_eq!(hit(w, "group-field-cancel").disabled(), Some(true));
                checkpoint("reload-refused");
                click(&hit(w, "group-field-review"), w, cx);
                if mode.contains("absent") {
                    Reviewed
                } else {
                    Reconciled
                }
            }
            Denied
                if message(w, "Group not saved: permission denied.")
                    || message(w, "Destination unavailable")
                    || message(w, "Session expired. Sign in again.") =>
            {
                assert_eq!(count, 1);
                if mode.contains("demoted") {
                    retained(w);
                    assert_eq!(hit(w, "group-name-g-z").disabled(), Some(true));
                }
                checkpoint("denied");
                w.remove_window();
                return;
            }
            _ => step,
        };
        schedule(v, mode, next, w);
    });
}
