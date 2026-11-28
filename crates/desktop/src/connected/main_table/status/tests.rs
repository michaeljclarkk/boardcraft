use super::*;
use core::prelude::v1::test;
use gpui_kit::test::TestWindowExt;

#[gpui_kit::test]
fn status_grid_returns_focus_to_the_item(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let item = Uuid::from_u128(42);
    let trigger = format!("status-{item}");
    let (window, _) = cx.update(|cx| {
        cx.set_reduce_motion(true);
        open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(1200.), px(1200.)),
                ))),
                ..Default::default()
            },
            cx,
            |w, cx| {
                cx.new(|cx| {
                    let board = Uuid::from_u128(8);
                    let groups = serde_json::from_value(serde_json::json!([{
                "id":"one", "name":"Group", "colour":"#579bfc", "revision":1, "collapsed":false,
                "items":[{"id":item, "title":"Review", "revision":12}]
            }])).unwrap();
                    let read = MainTableRead::from_response(groups, 1, 1, true, false).unwrap();
                    let mut t = MainTable::new(read, board, None, w, cx);
                    // Replace the deferred network visit with this already accepted snapshot.
                    t.status.visit = Uuid::from_u128(71);
                    t.status.accept(
                        StatusTable {
                            column_id: Uuid::from_u128(9),
                            title: "Status".into(),
                            choices: ["Working", "Done", "Stuck"]
                                .into_iter()
                                .map(|key| StatusChoice {
                                    key: key.into(),
                                    label: key.into(),
                                    color: "#579bfc".into(),
                                })
                                .collect(),
                            cells: vec![StatusCell {
                                item_id: item,
                                revision: 3,
                                value: Some("Done".into()),
                            }],
                        },
                        cx,
                    );
                    let mut view = Connected::new(0, None);
                    view.link_input = Some(cx.new(|cx| InputState::new(w, cx)));
                    view.screen = Screen::Navigation(Navigation {
                        client: EditClient::new(0),
                        page: Page::Items(board, Box::new(t)),
                        unread: 0,
                        share: None,
                        _people_events: Vec::new(),
                    });
                    view
                })
            },
        )
        .unwrap()
    });
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        w.click(trigger.clone(), cx);
        w.render_frame(cx);
        assert_eq!(w.find("status-choice-Done").focused(), Some(true));
        let working = w.find("status-choice-Working").bounds();
        let done = w.find("status-choice-Done").bounds();
        let empty = w.find("status-empty").bounds();
        assert_eq!(working.origin.y, done.origin.y);
        assert!(done.origin.x > working.origin.x && empty.origin.y > done.origin.y);
        assert!(w.try_find("status-clear").is_some());
        w.press("end", cx);
        assert_eq!(w.find("status-empty").focused(), Some(true));
        w.press("escape", cx);
        assert!(w.try_find("status-clear").is_none());
        assert_eq!(w.find(trigger.clone()).focused(), Some(true));
        w.press("enter", cx);
        assert_eq!(w.find("status-choice-Done").focused(), Some(true));
        w.press("enter", cx); // Confirming the accepted option closes without a write.
        assert!(w.try_find("status-clear").is_none());
        assert_eq!(w.find(trigger.clone()).focused(), Some(true));
    })
    .unwrap();
}
