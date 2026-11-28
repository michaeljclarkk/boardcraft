//! Interact with the production grouped view; all actions here finish before a write.
use super::Table;
use boardcraft_mobile_edit_client::MainTableRead;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, MouseButton, ScrollHandle, TestAppContext,
    WindowBounds, WindowOptions, px, size,
    test::{TestEventExt, TestQueryExt, TestWindowExt},
};
use serde_json::json;
use uuid::Uuid;

fn visit(cx: &mut TestAppContext, owner: bool) -> (AnyWindowHandle, Entity<Table>) {
    visit_titles(cx, owner, ["Ink", "Ink"])
}

fn visit_titles(
    cx: &mut TestAppContext,
    owner: bool,
    titles: [&str; 2],
) -> (AnyWindowHandle, Entity<Table>) {
    let group = |id: &str, item: u128, title: &str| json!({"id":id,"name":"Work","colour":"#443322","revision":1,"collapsed":false,"items":[{"id":Uuid::from_u128(item),"revision":1,"title":title}]});
    let groups = serde_json::from_value(json!([
        group("g-a", 59, titles[0]),
        group("g-b", 61, titles[1])
    ]))
    .unwrap();
    let read = MainTableRead::from_response(groups, 7, 1, owner, owner).unwrap();
    cx.update(gpui_kit::init);
    cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(1000.), px(800.)),
                ))),
                ..Default::default()
            },
            cx,
            |w, cx| {
                cx.new(|cx| {
                    Table::new(
                        "synthetic".into(),
                        Uuid::from_u128(201),
                        read,
                        ScrollHandle::new(),
                        w,
                        cx,
                    )
                })
            },
        )
        .unwrap()
    })
}

fn advance_search(cx: &mut TestAppContext, millis: u64) {
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(millis));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn selection_bar_stays_below_scroll_and_returns_dialog_focus(cx: &mut TestAppContext) {
    let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
    cx.simulate_window_resize(handle, size(px(681.), px(450.)));
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click(format!("check-{}", Uuid::from_u128(59)), cx);
        let bar = w.find("board-selection-bar").bounds();
        let scroll = w.find("content-scroll").bounds();
        assert!(bar.top() >= scroll.bottom());
        assert!(bar.bottom() <= px(450.));
        assert_eq!(w.find("board-selection-count").label(), Some("1 selected"));
        assert!(
            w.find("archive-selected").bounds().right()
                <= w.find("delete-selected").bounds().left()
        );
        assert!(
            w.find("delete-selected").bounds().right() <= w.find("move-selected").bounds().left()
        );
        w.scroll(
            "content-scroll",
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-320.))),
            cx,
        );
        assert!(table.read(cx).scroll.offset().y < px(0.));
        assert_eq!(w.find("board-selection-bar").bounds(), bar);
        w.click("archive-selected", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(table.read(cx).editor.is_some());
        assert_eq!(w.find("confirm").label(), Some("Archive"));
        w.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(table.read(cx).editor.is_none());
        assert_eq!(w.find("archive-selected").focused(), Some(true));
        w.press("tab", cx);
        assert_eq!(w.find("delete-selected").focused(), Some(true));
        w.press("tab", cx);
        assert_eq!(w.find("move-selected").focused(), Some(true));
        w.press("tab", cx);
        assert_eq!(w.find("clear-selected").focused(), Some(true));
        w.press("enter", cx);
        assert!(table.read(cx).selection.is_empty());
        assert!(w.try_find("board-selection-bar").is_none());
        assert_eq!(w.find("board-search-input").focused(), Some(true));
        assert!(table.read(cx).saves.is_empty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn selection_bar_keeps_readonly_commands_disabled_and_clear_available(cx: &mut TestAppContext) {
    let (handle, table) = visit(cx, false);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click(format!("check-{}", Uuid::from_u128(59)), cx);
        for action in ["archive-selected", "delete-selected", "move-selected"] {
            assert_eq!(w.find(action).disabled(), Some(true));
            w.click(action, cx);
            assert!(table.read(cx).editor.is_none());
        }
        w.click("clear-selected", cx);
        assert!(table.read(cx).selection.is_empty());
        assert_eq!(w.find("board-search-input").focused(), Some(true));
    })
    .unwrap();
}

#[gpui_kit::test]
fn item_search_debounces_and_clear_restores_hidden_selection_and_entry_draft(
    cx: &mut TestAppContext,
) {
    let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
    let ink = Uuid::from_u128(59);
    let sketch = Uuid::from_u128(61);
    let row = cx.update(|cx| table.read(cx).rows[&sketch].entity_id());
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click(format!("check-{sketch}"), cx);
        w.click("add-input-g-b", cx);
        w.input("Unsubmitted item", cx);
        w.click("board-search-input", cx);
        w.input(" iNk ", cx);
    })
    .unwrap();
    advance_search(cx, 249);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find(format!("item-{sketch}")).is_some());
    })
    .unwrap();
    advance_search(cx, 1);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find(format!("item-{ink}")).is_some());
        assert!(w.try_find(format!("item-{sketch}")).is_none());
        assert_eq!(table.read(cx).selection, [sketch]);
        w.click_with_modifiers(format!("check-{ink}"), gpui_kit::Modifiers::shift(), cx);
        assert_eq!(table.read(cx).selection, [sketch]);
        assert_eq!(
            table.read(cx).message,
            "An item left this view. Reload the board and select the range again."
        );
        w.click("board-search-input", cx);
        w.press("ctrl-a", cx);
        w.input("QXZ", cx);
    })
    .unwrap();
    advance_search(cx, 250);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find(format!("item-{ink}")).is_none());
        assert_ne!(w.find("new-item").disabled(), Some(true));
        w.click("clear-board-search", cx);
    })
    .unwrap();
    advance_search(cx, 250);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(w.find("board-search-input").value(), Some(""));
        assert_eq!(w.find("board-search-input").focused(), Some(true));
        assert!(w.try_find(format!("item-{ink}")).is_some());
        assert_eq!(w.find(format!("check-{sketch}")).checked(), Some(true));
        assert_eq!(w.find("add-input-g-b").value(), Some("Unsubmitted item"));
        assert_eq!(table.read(cx).rows[&sketch].entity_id(), row);
        assert!(table.read(cx).saves.is_empty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn newest_search_wins_and_invalid_length_preserves_the_last_results(cx: &mut TestAppContext) {
    let (handle, table) = visit_titles(cx, false, ["Ink", "Sketch"]);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("board-search-input", cx);
        w.input("Ink", cx);
    })
    .unwrap();
    advance_search(cx, 100);
    cx.update_window(handle, |_, w, cx| {
        w.press("ctrl-a", cx);
        w.input("Sketch", cx);
    })
    .unwrap();
    advance_search(cx, 150);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(table.read(cx).result_search.is_empty());
    })
    .unwrap();
    advance_search(cx, 100);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(table.read(cx).result_search, "sketch");
        assert!(
            w.try_find(format!("item-{}", Uuid::from_u128(59)))
                .is_none()
        );
        w.press("ctrl-a", cx);
        // The reference's limit counts UTF-16 units, not Unicode scalar values.
        w.input(&"😀".repeat(121), cx);
    })
    .unwrap();
    advance_search(cx, 250);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(table.read(cx).search_invalid);
        assert_eq!(table.read(cx).result_search, "sketch");
        assert!(
            w.try_find(format!("item-{}", Uuid::from_u128(61)))
                .is_some()
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn filtered_group_selection_keeps_hidden_rows_and_reference_header_count(cx: &mut TestAppContext) {
    let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
    let ink = Uuid::from_u128(59);
    let sketch = Uuid::from_u128(61);
    cx.update_window(handle, |_, w, cx| {
        table.update(cx, |table, cx| {
            let groups = serde_json::from_value(json!([{
                "id":"g-a", "name":"Work", "colour":"#443322",
                "revision":1, "collapsed":false,
                "items":[
                    {"id":ink,"revision":1,"title":"Ink"},
                    {"id":sketch,"revision":1,"title":"Sketch"}
                ]
            }]))
            .unwrap();
            table.read = MainTableRead::from_response(groups, 7, 1, true, true).unwrap();
            table.mount(w, cx);
            // Membership has already been resolved for this visit. This test
            // exercises the local checkbox gesture, not browser transport.
            table.selection_resolved = true;
        });
        w.render_frame(cx);
        w.click(format!("check-{sketch}"), cx);
        w.click("board-search-input", cx);
        w.input("Ink", cx);
    })
    .unwrap();
    advance_search(cx, 250);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find(format!("item-{sketch}")).is_none());
        assert_eq!(w.find(format!("check-{ink}")).checked(), Some(false));
        // This apparently surprising state matches Tuesday: the hidden Sketch
        // contributes one selected parent against one matching item (Ink).
        assert_eq!(w.find("check-group-g-a").checked(), Some(true));
        w.click("check-group-g-a", cx);
        assert_eq!(table.read(cx).selection, [sketch]);
        w.click(format!("check-{ink}"), cx);
        assert_eq!(table.read(cx).selection, [sketch, ink]);
        assert_eq!(w.find("check-group-g-a").checked(), Some(false));
        w.click("board-search-input", cx);
        w.press("ctrl-a", cx);
        w.press("backspace", cx);
    })
    .unwrap();
    advance_search(cx, 250);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(w.find("check-group-g-a").checked(), Some(true));
        assert_eq!(w.find(format!("check-{sketch}")).checked(), Some(true));
        w.click("check-group-g-a", cx);
        assert!(table.read(cx).selection.is_empty());
        assert!(table.read(cx).saves.is_empty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn board_tools_keep_reload_reachable_when_a_conflict_requires_reconciliation(
    cx: &mut TestAppContext,
) {
    let (handle, table) = visit(cx, true);
    cx.update_window(handle, |_, w, cx| {
        table.update(cx, |table, cx| {
            table.reconcile = true;
            cx.notify();
        });
        w.render_frame(cx);
        assert_eq!(w.find("new-item").disabled(), Some(true));
        w.click("board-tools-menu", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_ne!(w.find_by_label("Reload board").disabled(), Some(true));
        let add = w.find_by_label("Add group").bounds().center();
        w.pointer_down(add, MouseButton::Left, cx);
        w.pointer_up(add, MouseButton::Left, cx);
        assert!(table.read(cx).editor.is_none());
        let recovery = w
            .find_by_label("Archived & deleted content")
            .bounds()
            .center();
        w.pointer_down(recovery, MouseButton::Left, cx);
        w.pointer_up(recovery, MouseButton::Left, cx);
        assert!(table.read(cx).inventory.is_none());
        w.press("escape", cx);
        assert!(table.read(cx).reconcile);
        assert!(!table.read(cx).pending);
    })
    .unwrap();
}

#[gpui_kit::test]
fn new_item_focuses_existing_entry_and_new_group_menu_opens_real_editor(cx: &mut TestAppContext) {
    let (handle, table) = visit(cx, true);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("new-item", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(w.find("add-input-g-a").focused(), Some(true));
        w.click("new-item-options", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.find_by_label("New group of items").visible());
        w.press("down", cx);
        w.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(table.read(cx).editor.is_some());
        assert!(w.try_find("group-name").is_some());
        w.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(table.read(cx).editor.is_none());
        w.click("board-tools-menu", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.find_by_label("Reload board").visible());
        assert!(w.find_by_label("Archived & deleted content").visible());
        w.press("escape", cx);
        assert!(!table.read(cx).pending);
    })
    .unwrap();
}

#[gpui_kit::test]
fn viewer_fold_keeps_the_hidden_anchor_and_the_other_duplicate_visible(cx: &mut TestAppContext) {
    let (handle, t) = visit(cx, false);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        let first = format!("check-{}", Uuid::from_u128(59));
        let second = format!("check-{}", Uuid::from_u128(61));
        w.click(second.clone(), cx);
        assert_eq!(t.read(cx).anchor, Some(Uuid::from_u128(61)));
        w.click("fold-g-b", cx);
        assert_eq!(w.find("fold-g-b").expanded(), Some(false));
        assert!(w.try_find(second.clone()).is_none());
        assert_eq!(w.find(first).checked(), Some(false));
        assert_eq!(t.read(cx).selection, [Uuid::from_u128(61)]);
        w.click("clear-selected", cx);
        assert!(t.read(cx).selection.is_empty());
        assert_eq!(t.read(cx).anchor, Some(Uuid::from_u128(61)));
        w.click("fold-g-b", cx);
        assert_eq!(w.find(second).checked(), Some(false));
        assert!(!t.read(cx).pending);
    })
    .unwrap();
}

#[gpui_kit::test]
fn f2_edits_only_the_focused_duplicate_and_escape_returns_to_it(cx: &mut TestAppContext) {
    let (handle, t) = visit(cx, true);
    let first = Uuid::from_u128(59);
    let second = Uuid::from_u128(61);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click(format!("item-{second}"), cx);
        w.press("f2", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(w.find(format!("edit-name-{second}")).value(), Some("Ink"));
        assert!(w.try_find(format!("edit-name-{first}")).is_none());
        w.input(" draft", cx);
        w.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(w.find(format!("item-{second}")).focused(), Some(true));
        assert!(w.try_find(format!("edit-name-{second}")).is_none());
        let t = t.read(cx);
        assert!(t.saves.is_empty());
        assert_eq!(t.rows[&first].read(cx).accepted_title(), "Ink");
        assert_eq!(t.rows[&second].read(cx).accepted_title(), "Ink");
    })
    .unwrap();
}

#[gpui_kit::test]
fn editing_the_second_heading_and_cancelling_keeps_both_groups(cx: &mut TestAppContext) {
    let (handle, t) = visit(cx, true);
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("heading-g-b", cx);
        assert!(t.read(cx).editor.is_none());
        w.double_click("heading-g-b", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(w.find("group-name").focused(), Some(true));
        assert_eq!(w.find("group-name").value(), Some("Work"));
        w.input(" revised", cx);
        assert_eq!(w.find("group-name").value(), Some("Work revised"));
        w.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("group-name").is_none());
        let t = t.read(cx);
        assert!(t.editor.is_none() && !t.pending);
        let groups: Vec<_> = t
            .read
            .groups()
            .iter()
            .map(|g| (g.id(), g.name(), g.revision()))
            .collect();
        assert_eq!(groups, [("g-a", "Work", 1), ("g-b", "Work", 1)]);
    })
    .unwrap();
}
