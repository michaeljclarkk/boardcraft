//! Search operates the mounted native table; hidden rows keep their visit state.
use super::{
    Connected, EditClient, InputState, MainTable, MainTableRead, Navigation, Page, Screen,
};
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    px, size, test::TestWindowExt,
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
