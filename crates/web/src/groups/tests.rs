//! Interact with the production grouped view; all actions here finish before a write.
use super::Table;
use boardcraft_mobile_edit_client::MainTableRead;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, ScrollHandle, TestAppContext, WindowBounds,
    WindowOptions, px, size, test::TestWindowExt,
};
use serde_json::json;
use uuid::Uuid;

fn visit(cx: &mut TestAppContext, owner: bool) -> (AnyWindowHandle, Entity<Table>) {
    let group = |id: &str, item: u128| json!({"id":id,"name":"Work","colour":"#443322","revision":1,"collapsed":false,"items":[{"id":Uuid::from_u128(item),"revision":1,"title":"Ink"}]});
    let groups = serde_json::from_value(json!([group("g-a", 59), group("g-b", 61)])).unwrap();
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
