//! Search operates the mounted native table; hidden rows keep their visit state.
use super::{
    Connected, EditClient, InputState, MainTable, MainTableRead, Navigation, Page, Screen,
};
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    px, size,
    test::{TestQueryExt, TestWindowExt},
};
use uuid::Uuid;

fn table(view: &Connected) -> &MainTable {
    let Screen::Navigation(nav) = &view.screen else {
        panic!("navigation")
    };
    let Page::Items(_, table) = &nav.page else {
        panic!("table")
    };
    table
}

fn visit(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Connected>) {
    cx.update(gpui_kit::init);
    cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(), size(px(1200.), px(1000.)),
                ))),
                ..Default::default()
            }, cx, |w, cx| cx.new(|cx| {
                let board = Uuid::from_u128(8);
                let groups = serde_json::from_value(serde_json::json!([
                    {"id":"one", "name":"New group", "colour":"#579bfc", "revision":1, "collapsed":false,
                     "items":[{"id":Uuid::from_u128(42), "title":"Sketch", "revision":1},
                              {"id":Uuid::from_u128(17), "title":"Ink", "revision":1}]},
                    {"id":"empty", "name":"Reserve", "colour":"#579bfc", "revision":1, "collapsed":false, "items":[]}
                ])).unwrap();
                let read = MainTableRead::from_response(groups, 1, 1, true, true).unwrap();
                let t = MainTable::new(read, board, None, w, cx);
                let mut view = Connected::new(0, None);
                view.link_input = Some(cx.new(|cx| InputState::new(w, cx)));
                view.screen = Screen::Navigation(Navigation {
                    client: EditClient::new(0), page: Page::Items(board, Box::new(t)),
                    unread: 0, share: None, workspace: None, _people_events: Vec::new(),
                });
                view
            }),
        ).unwrap()
    })
}

fn advance(cx: &mut TestAppContext, millis: u64) {
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(millis));
    cx.run_until_parked();
}

fn selected(view: &Entity<Connected>, cx: &App) -> Vec<Uuid> {
    table(view.read(cx)).selection.ids.clone()
}

#[gpui_kit::test]
fn compact_header_and_link_footer_keep_actions_reachable(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    cx.simulate_window_resize(window, size(px(681.), px(1025.)));
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.find("share").bounds().bottom() < w.find("new-item").bounds().top());
        let back = w.find("board-list").bounds();
        let reload = w.find("reload-table").bounds();
        let inbox = w.find("notifications").bounds();
        assert_eq!(back.center().y, reload.center().y);
        assert_eq!(reload.center().y, inbox.center().y);
        let address = w.find("board-link").bounds();
        let open = w.find("open-link").bounds();
        assert_eq!(address.center().y, open.center().y);
        assert!(address.right() < open.left());
        w.click("share", cx);
        assert_eq!(
            w.find("share-address").value(),
            Some("http://127.0.0.1:0/boards/00000000-0000-0000-0000-000000000008")
        );
        w.click("board-link", cx);
        w.input("invalid address", cx);
        w.click("open-link", cx);
        assert_eq!(
            view.read(cx).message,
            "Enter a board link from this connection."
        );
        assert_eq!(w.find("board-link").focused(), Some(true));
        assert_eq!(w.find("board-link").value(), Some("invalid address"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn search_waits_for_idle_and_restores_hidden_selection_and_entry_draft(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    let sketch = Uuid::from_u128(42);
    let ink = Uuid::from_u128(17);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        w.click(format!("select-{sketch}"), cx);
        w.click("add-one", cx);
        w.input("Unsubmitted", cx);
        w.click("view-filter", cx);
        w.input(" iNk ", cx);
        w.press("enter", cx);
    })
    .unwrap();
    advance(cx, 249);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find(format!("select-{sketch}")).is_some());
        assert!(table(view.read(cx)).filter.is_empty());
    })
    .unwrap();
    advance(cx, 1);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find(format!("select-{sketch}")).is_none());
        assert_eq!(w.find(format!("select-{ink}")).checked(), Some(false));
        assert!(visible_text(w, "1 matching items"));
        assert!(visible_text(w, "1 item"));
        assert!(w.try_find("fold-empty").is_none());
        assert_eq!(selected(&view, cx), [sketch]);
        // Match Tuesday's physical-group checkbox, including its hidden-selection count.
        assert_eq!(w.find("select-group-one").checked(), Some(true));
        w.press("ctrl-a", cx);
        w.input("QXZ", cx);
    })
    .unwrap();
    advance(cx, 250);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(visible_text(w, "No results were found"));
        w.click("clear-board-search", cx);
    })
    .unwrap();
    advance(cx, 250);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(w.find("view-filter").value(), Some(""));
        assert_eq!(w.find("view-filter").focused(), Some(true));
        assert_eq!(w.find(format!("select-{sketch}")).checked(), Some(true));
        assert_eq!(w.find("add-one").value(), Some("Unsubmitted"));
        assert!(w.try_find("fold-empty").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn newer_search_cancels_older_and_invalid_utf16_keeps_results(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        w.click("view-filter", cx);
        w.input("Ink", cx);
    })
    .unwrap();
    advance(cx, 100);
    cx.update_window(window, |_, w, cx| {
        w.press("ctrl-a", cx);
        w.input("Sketch", cx);
    })
    .unwrap();
    advance(cx, 150);
    cx.update(|cx| assert!(table(view.read(cx)).filter.is_empty()));
    advance(cx, 100);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(table(view.read(cx)).filter, "Sketch");
        w.press("ctrl-a", cx);
        w.input(&"😀".repeat(121), cx);
    })
    .unwrap();
    advance(cx, 250);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(table(view.read(cx)).search_invalid);
        assert_eq!(table(view.read(cx)).filter, "Sketch");
        assert!(visible_text(w, "Search must be 240 characters or fewer."));
        w.press("ctrl-a", cx);
        w.input("QXZ", cx);
    })
    .unwrap();
    advance(cx, 250);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(!table(view.read(cx)).search_invalid);
        w.click("new-item", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(w.find("add-one").focused(), Some(true));
        assert!(table(view.read(cx)).filter.is_empty());
    })
    .unwrap();
}

fn visible_text(w: &gpui_kit::Window, expected: &str) -> bool {
    gpui_kit::base::test_support::texts(w)
        .iter()
        .any(|text| text.visible() && text.text() == expected)
}

#[gpui_kit::test]
fn filtered_group_selection_never_changes_hidden_members(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    let sketch = Uuid::from_u128(42);
    let ink = Uuid::from_u128(17);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        w.click(format!("select-{sketch}"), cx);
        w.click("view-filter", cx);
        w.input("Ink", cx);
    })
    .unwrap();
    advance(cx, 250);
    cx.update_window(window, |_, w, cx| {
        // The fixture is an accepted complete selection read, avoiding HTTP in this UI test.
        view.update(cx, |view, _| {
            let Screen::Navigation(nav) = &mut view.screen else {
                unreachable!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                unreachable!()
            };
            table.selection_cache = true;
        });
        w.render_frame(cx);
        w.click("select-group-one", cx);
        assert_eq!(selected(&view, cx), [sketch]);
        w.click(format!("select-{ink}"), cx);
        assert_eq!(selected(&view, cx), [sketch, ink]);
        assert_eq!(w.find("select-group-one").checked(), Some(false));
        w.click("selection-clear", cx);
        w.click("select-group-one", cx);
        assert_eq!(selected(&view, cx), [ink]);
        w.click("select-group-one", cx);
        assert!(selected(&view, cx).is_empty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn selection_bar_stays_at_bottom_and_clear_returns_focus_after_resize(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    let sketch = Uuid::from_u128(42);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        w.click(format!("select-{sketch}"), cx);
        let bar = w.find("board-selection-bar").bounds();
        let viewport = w.find("navigation-content").bounds();
        assert!(bar.top() > w.find("add-empty").bounds().bottom());
        assert!(bar.bottom() < viewport.bottom());
        assert!(bar.bottom() > viewport.bottom() - px(24.));
        assert!((bar.center().x - viewport.center().x).abs() <= px(1.));
        assert!(
            w.find("selection-archive").bounds().left()
                < w.find("selection-delete").bounds().left()
        );
        assert!(
            w.find("selection-delete").bounds().left() < w.find("selection-move").bounds().left()
        );
        assert_eq!(w.find("selection-archive").label(), Some("Archive"));
        assert_eq!(w.find("selection-delete").label(), Some("Delete"));
        assert_eq!(w.find("selection-move").label(), Some("Move to"));
    })
    .unwrap();
    cx.simulate_window_resize(window, size(px(681.), px(500.)));
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        let before = w.find("board-selection-bar").bounds();
        assert_eq!(
            before.left(),
            w.find("navigation-content").bounds().left() + w.rem_size()
        );
        w.scroll(
            "navigation-content",
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-600.))),
            cx,
        );
        assert_eq!(w.find("board-selection-bar").bounds(), before);
        for action in [
            "selection-archive",
            "selection-delete",
            "selection-move",
            "selection-clear",
        ] {
            assert!(w.find(action).visible());
        }
        w.click("selection-clear", cx);
        assert!(selected(&view, cx).is_empty());
        assert!(w.try_find("board-selection-bar").is_none());
        // The focused row may be above the scroll viewport, but its retained focus
        // remains an actionable keyboard destination after Clear removes itself.
        assert_eq!(w.find(format!("title-{sketch}")).focused(), Some(true));
    })
    .unwrap();
}

#[gpui_kit::test]
fn selection_bar_preserves_commands_and_permission_guards(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    let sketch = Uuid::from_u128(42);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        w.click(format!("select-{sketch}"), cx);
        w.click("selection-archive", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(table(view.read(cx)).move_dialog.is_some());
        assert_eq!(w.find("move-confirm").label(), Some("Archive"));
        w.click("move-cancel", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(table(view.read(cx)).move_dialog.is_none());
        assert_eq!(w.find("selection-archive").focused(), Some(true));
        assert_eq!(selected(&view, cx), [sketch]);
        view.update(cx, |v, cx| {
            let Screen::Navigation(nav) = &mut v.screen else {
                unreachable!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                unreachable!()
            };
            table.manageable = false;
            table.editable = false;
            cx.notify();
        });
        w.render_frame(cx);
        for action in ["selection-archive", "selection-delete", "selection-move"] {
            assert_eq!(w.find(action).disabled(), Some(true));
            w.click(action, cx);
            assert!(table(view.read(cx)).move_dialog.is_none());
        }
        w.click("selection-clear", cx);
        assert!(selected(&view, cx).is_empty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn board_tools_open_group_and_automation_controls_and_leave_reload_reachable(
    cx: &mut TestAppContext,
) {
    let (window, view) = visit(cx);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        w.click("board-tools-menu", cx);
        let add = w.find_by_label("Add group");
        w.within("popup-menu")
            .click(add.path().last().unwrap().clone(), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(table(view.read(cx)).group_dialog.is_some());
        assert_eq!(w.find("group-field-name").focused(), Some(true));
        w.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(table(view.read(cx)).group_dialog.is_none());
        assert_eq!(w.find("board-tools-menu").focused(), Some(true));
        w.click("board-tools-menu", cx);
        let automate = w.find_by_label("Automations");
        w.within("popup-menu")
            .click(automate.path().last().unwrap().clone(), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(table(view.read(cx)).automations_open);
        assert!(w.find("automation-name").visible());
        view.update(cx, |v, cx| {
            let Screen::Navigation(nav) = &mut v.screen else {
                unreachable!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                unreachable!()
            };
            table.reconcile = true;
            cx.notify();
        });
        w.render_frame(cx);
        w.click("board-tools-menu", cx);
        // This Kit menu does not expose disabled through its accessibility properties;
        // dispatch its actual pointer path and verify that the guard refuses the editor.
        let add = w.within("popup-menu").find_by_label("Add group");
        w.within("popup-menu")
            .click(add.path().last().unwrap().clone(), cx);
        assert!(table(view.read(cx)).group_dialog.is_none());
        let reload = w.within("popup-menu").find_by_label("Reload board");
        w.within("popup-menu")
            .click(reload.path().last().unwrap().clone(), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(table(view.read(cx)).reconcile);
        assert_eq!(
            view.read(cx).message,
            "Could not reload board. Retry loading before editing again."
        );
        assert!(!table(view.read(cx)).pending);
    })
    .unwrap();
}
