use super::*;
use core::prelude::v1::test;
use gpui_kit::test::{ElementSnapshot, TestQueryExt, TestWindowExt};

fn text(w: &Window, expected: &str) -> Option<ElementSnapshot> {
    w.elements()
        .into_iter()
        .find(|e| e.value() == Some(expected))
}

#[gpui_kit::test]
fn read_only_item_menu_opens_retryable_log_and_restores_focus(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let item = Uuid::from_u128(42);
    let trigger = format!("item-menu-{item}");
    let (window, _) = cx.update(|cx| {
        cx.set_reduce_motion(true);
        let bounds = Bounds::new(Default::default(), size(px(1200.), px(1200.)));
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };
        let groups = serde_json::from_value(serde_json::json!([{
            "id": "one", "name": "Group", "colour": "#579bfc", "revision": 1, "collapsed": false,
            "items": [{"id": item, "title": "Review", "revision": 1}]
        }]))
        .unwrap();
        let read = MainTableRead::from_response(groups, 1, 1, false, false).unwrap();
        open_window(options, cx, |w, cx| {
            cx.new(|cx| {
                let board = Uuid::from_u128(8);
                let table = MainTable::new(read, board, None, w, cx);
                let mut view = Connected::new(0, None);
                view.link_input = Some(cx.new(|cx| InputState::new(w, cx)));
                view.screen = Screen::Navigation(Navigation {
                    client: EditClient::new(0),
                    page: Page::Items(board, Box::new(table)),
                    unread: 0,
                    share: None,
                    _people_events: Vec::new(),
                });
                view
            })
        })
        .unwrap()
    });
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        w.click(trigger.clone(), cx);
        let menu_item = w.find_by_label("Activity Log");
        w.click(menu_item.path().last().unwrap().clone(), cx);
        assert!(text(w, "Loading captured activity…").unwrap().visible());
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(text(w, "Activity could not be loaded.").unwrap().visible());
        w.click("retry-activity", cx);
        assert!(text(w, "Loading captured activity…").unwrap().visible());
        w.press("escape", cx);
        assert!(w.try_find("activity-search").is_none());
        assert_eq!(w.find(trigger.clone()).focused(), Some(true));
    })
    .unwrap();
}

#[gpui_kit::test]
fn activity_search_shows_newest_first_values_and_no_matches(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, log) = cx.update(|cx| {
        open_window(WindowOptions::default(), cx, |w, cx| {
            cx.new(|cx| {
                let mut log = ActivityLog::new(
                    EditClient::new(0),
                    Uuid::from_u128(8),
                    Uuid::from_u128(42),
                    w,
                    cx,
                );
                log.read = ActivityRead::Ready(vec![]);
                log
            })
        })
        .unwrap()
    });
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(
            text(w, "No captured activity for this item")
                .unwrap()
                .visible()
        );
        log.update(cx, |log, cx| {
            let change = |revision, before: &str, after: &str, actor: &str| ActivityEntry {
                board_id: log.board,
                item_id: log.item,
                actor_id: Uuid::from_u128(9),
                actor_name: actor.into(),
                occurred_at: "2026-11-28T11:00:00Z".into(),
                field: "name".into(),
                before: before.into(),
                after: after.into(),
                revision,
            };
            log.read = ActivityRead::Ready(vec![
                change(3, "Review", "Ready", "Casey"),
                change(2, "", "Review", "Jess"),
            ]);
            cx.notify();
        });
        w.render_frame(cx);
        assert!(text(w, "Casey").unwrap().bounds().top() < text(w, "Jess").unwrap().bounds().top());
        let latest = w.within("activity-3-name");
        assert_eq!(latest.find("previous-value").value(), Some("Review"));
        assert_eq!(latest.find("new-value").value(), Some("Ready"));
        assert!(text(w, "2026-11-28 11:00:00 UTC").unwrap().visible());
        assert!(text(w, "Changed name · Revision 3").unwrap().visible());
        w.click("activity-search", cx);
        w.input("jess", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(text(w, "Jess").unwrap().visible());
        assert!(text(w, "Casey").is_none());
        w.input("x", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(text(w, "No matching activity").unwrap().visible());
    })
    .unwrap();
}
