use super::*;
use core::prelude::v1::test;
use gpui_kit::test::TestWindowExt;

fn visit(
    cx: &mut TestAppContext,
    value: Option<&str>,
    editable: bool,
) -> (AnyWindowHandle, Entity<Connected>) {
    cx.update(gpui_kit::init);
    let item = Uuid::from_u128(42);
    cx.update(|cx| {
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
                    let read = MainTableRead::from_response(groups, 1, 1, editable, false).unwrap();
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
                                value: value.map(str::to_owned),
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
                        workspace: None,
                        _people_events: Vec::new(),
                    });
                    view
                })
            },
        )
        .unwrap()
    })
}

fn table(view: &Connected) -> &MainTable {
    let Screen::Navigation(nav) = &view.screen else {
        panic!("navigation");
    };
    let Page::Items(_, table) = &nav.page else {
        panic!("table");
    };
    table
}

#[gpui_kit::test]
fn status_grid_returns_focus_to_the_item(cx: &mut TestAppContext) {
    let (window, view) = visit(cx, Some("Done"), true);
    let trigger = format!("status-{}", Uuid::from_u128(42));
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        let row = w
            .find(format!("selected-row-{}", Uuid::from_u128(42)))
            .bounds();
        let cell = w.find(trigger.clone()).bounds();
        assert!(
            (row.size.height - cell.size.height).abs() <= px(1.),
            "row {row:?}, cell {cell:?}"
        );
        w.click(trigger.clone(), cx);
        w.render_frame(cx);
        assert_eq!(w.find("status-choice-Done").focused(), Some(true));
        assert_eq!(w.find("status-choice-Done").checked(), Some(true));
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
        assert!(!table(view.read(cx)).pending);
        assert!(table(view.read(cx)).status.notice.is_none());
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(!table(view.read(cx)).status.requires_reload));
}

#[gpui_kit::test]
fn empty_status_keeps_clear_action_and_both_noop_choices_close_without_writing(
    cx: &mut TestAppContext,
) {
    let (window, view) = visit(cx, None, true);
    let trigger = format!("status-{}", Uuid::from_u128(42));
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        w.click(trigger.clone(), cx);
        assert_eq!(w.find("status-empty").focused(), Some(true));
        assert_eq!(w.find("status-empty").checked(), Some(true));
        assert_ne!(w.find("status-clear").disabled(), Some(true));
        w.press("enter", cx);
        assert!(w.try_find("status-clear").is_none());
        assert_eq!(w.find(trigger.clone()).focused(), Some(true));
        w.press("enter", cx);
        w.press("tab", cx);
        assert_eq!(w.find("status-clear").focused(), Some(true));
        w.press("enter", cx);
        assert!(w.try_find("status-clear").is_none());
        assert_eq!(w.find(trigger).focused(), Some(true));
        assert!(table(view.read(cx)).status.notice.is_none());
        assert!(!table(view.read(cx)).pending);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let status = &table(view.read(cx)).status;
        assert!(!status.requires_reload);
        let ColumnRead::Ready { cells, .. } = &status.read else {
            panic!()
        };
        assert_eq!(cells[&Uuid::from_u128(42)].saved.revision, 3);
        assert_eq!(cells[&Uuid::from_u128(42)].saved.value, None);
    });
}

#[gpui_kit::test]
fn status_outside_click_keeps_destination_focus_and_viewer_cannot_open_picker(
    cx: &mut TestAppContext,
) {
    let (window, view) = visit(cx, Some("Done"), true);
    let trigger = format!("status-{}", Uuid::from_u128(42));
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        w.click(trigger.clone(), cx);
        w.click("view-filter", cx);
        assert!(w.try_find("status-clear").is_none());
        assert_eq!(w.find("view-filter").focused(), Some(true));
        view.update(cx, |view, cx| {
            let Screen::Navigation(nav) = &mut view.screen else {
                panic!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                panic!()
            };
            table.editable = false;
            cx.notify();
        });
        w.render_frame(cx);
        assert_eq!(w.find(trigger.clone()).disabled(), Some(true));
        w.click(trigger, cx);
        assert!(w.try_find("status-clear").is_none());
        assert!(!table(view.read(cx)).pending);
    })
    .unwrap();
}

#[gpui_kit::test]
fn unavailable_status_recovery_keeps_accepted_cell_and_requires_fresh_read(
    cx: &mut TestAppContext,
) {
    let (window, view) = visit(cx, Some("Done"), true);
    let item = Uuid::from_u128(42);
    let trigger = format!("status-{item}");
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        let row = w.find(format!("selected-row-{item}")).bounds();
        w.click(trigger.clone(), cx);
        w.click("status-choice-Stuck", cx);
        assert!(table(view.read(cx)).pending);
        assert_eq!(table(view.read(cx)).status.pending, Some(item));
        assert!(table(view.read(cx)).status.notice_for(item).is_none());
        assert_eq!(w.find(format!("selected-row-{item}")).bounds(), row);
        assert_eq!(
            w.find(trigger.clone()).label(),
            Some("Status for Review: Saving…")
        );
        assert!(w.try_find("status-clear").is_none());
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(!table(view.read(cx)).pending);
        assert_eq!(table(view.read(cx)).status.pending, None);
        assert!(table(view.read(cx)).status.requires_reload);
        assert_eq!(
            w.find(trigger.clone()).label(),
            Some("Status for Review: Done")
        );
        assert_eq!(w.find(trigger.clone()).disabled(), Some(true));
        w.click(trigger.clone(), cx);
        assert!(w.try_find("status-clear").is_none());
        assert!(
            table(view.read(cx))
                .status
                .notice_for(item)
                .unwrap()
                .starts_with("Status not confirmed.")
        );
        view.update(cx, |view, cx| {
            let visit = table(view).status.visit;
            // This is the successful authorized response to a later deliberate
            // read. The refused write is never replayed by the screen.
            view.finish_status_read(
                visit,
                Ok(StatusTable {
                    column_id: Uuid::from_u128(9),
                    title: "Status".into(),
                    choices: vec![StatusChoice {
                        key: "Done".into(),
                        label: "Done".into(),
                        color: "#00c875".into(),
                    }],
                    cells: vec![StatusCell {
                        item_id: item,
                        revision: 3,
                        value: Some("Done".into()),
                    }],
                }),
                w,
                cx,
            );
        });
        w.render_frame(cx);
        assert_ne!(w.find(trigger.clone()).disabled(), Some(true));
        w.click(trigger, cx);
        assert_eq!(w.find("status-choice-Done").focused(), Some(true));
        w.press("enter", cx);
        assert!(!table(view.read(cx)).pending);
    })
    .unwrap();
}
