use super::super::tests::{advance, table};
use super::*;
use boardcraft_mobile_edit_client::{StatusCell, StatusTable};
use core::prelude::v1::test;
use gpui_kit::test::TestWindowExt;

fn visit(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Connected>) {
    let (window, view) = super::super::tests::visit(cx);
    cx.run_until_parked();
    cx.update(|cx| {
        cx.set_reduce_motion(true);
        view.update(cx, |view, cx| {
            let Screen::Navigation(nav) = &mut view.screen else {
                unreachable!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                unreachable!()
            };
            table.status.accept(
                StatusTable {
                    column_id: Uuid::from_u128(9),
                    title: "Status".into(),
                    choices: ["Working on it", "Done", "Stuck"]
                        .into_iter()
                        .map(|key| StatusChoice {
                            key: key.into(),
                            label: key.into(),
                            color: "#579bfc".into(),
                        })
                        .collect(),
                    cells: vec![
                        StatusCell {
                            item_id: Uuid::from_u128(42),
                            revision: 1,
                            value: Some("Done".into()),
                        },
                        StatusCell {
                            item_id: Uuid::from_u128(17),
                            revision: 1,
                            value: None,
                        },
                    ],
                },
                cx,
            );
            cx.notify();
        });
    });
    (window, view)
}

#[gpui_kit::test]
fn preview_waits_then_discard_restores_hidden_selection_and_focus(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    let ink = Uuid::from_u128(17);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        w.click(format!("select-{ink}"), cx);
        w.click("quick-filters", cx);
        for id in [
            "filter-status-Working on it",
            "filter-status-Done",
            "filter-status-Stuck",
            "filter-status-empty",
        ] {
            let bounds = w.find(id).bounds();
            assert_eq!(bounds.size.width, rems(9.375).to_pixels(w.rem_size()));
            assert_eq!(bounds.size.height, rems(2.).to_pixels(w.rem_size()));
        }
        w.click("filter-status-Done", cx);
    })
    .unwrap();
    advance(cx, 199);
    cx.update(|cx| assert_eq!(table(view.read(cx)).filters, ViewFilters::default()));
    advance(cx, 1);
    cx.update(|cx| {
        let table = table(view.read(cx));
        assert_eq!(table.filters.status_values, ["Done"]);
        assert_eq!(table.order(cx), [Uuid::from_u128(42)]);
        assert_eq!(table.selection.ids, [ink]);
    });
    cx.update_window(window, |_, w, cx| {
        w.click("filter-status-empty", cx);
        assert_eq!(w.find("filter-status-Done").checked(), Some(false));
        assert_eq!(w.find("filter-status-empty").checked(), Some(true));
        w.click("filter-discard", cx);
        assert_eq!(w.find("quick-filters").focused(), Some(true));
    })
    .unwrap();
    advance(cx, 250);
    cx.update(|cx| {
        let table = table(view.read(cx));
        assert_eq!(table.filters, ViewFilters::default());
        assert_eq!(table.order(cx), [Uuid::from_u128(42), ink]);
        assert_eq!(table.selection.ids, [ink]);
    });
}

#[gpui_kit::test]
fn close_and_apply_keep_latest_draft_and_last_group_cannot_be_removed(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        w.click("quick-filters", cx);
        w.click("filter-status-empty", cx);
        w.click("filter-status-Done", cx);
        assert_eq!(w.find("filter-status-empty").checked(), Some(false));
        w.click("filter-group-empty", cx);
        assert_eq!(w.find("filter-group-one").disabled(), Some(true));
        w.click("filter-close", cx);
        let current = table(view.read(cx));
        assert_eq!(current.filters.status_values, ["Done"]);
        assert_eq!(current.filters.group_ids, ["one"]);
        assert!(!current.filters.include_empty_status);
        assert_eq!(w.find("quick-filters").focused(), Some(true));
        w.click("quick-filters", cx);
        w.click("filter-status-empty", cx);
        w.click("filter-apply", cx);
        let table = table(view.read(cx));
        assert!(table.filters.status_values.is_empty());
        assert!(table.filters.include_empty_status);
        assert_eq!(table.order(cx), [Uuid::from_u128(17)]);
    })
    .unwrap();
    advance(cx, 250);
    cx.update(|cx| assert!(table(view.read(cx)).filters.include_empty_status));
}

fn saved(id: Uuid) -> SavedView {
    SavedView {
        id,
        name: "Review".into(),
        filter: "Sketch".into(),
        shared: false,
        owner: Uuid::from_u128(77),
        filters: ViewFilters {
            status_values: vec!["Done".into()],
            include_empty_status: false,
            group_ids: vec!["one".into()],
        },
    }
}

#[gpui_kit::test]
fn saved_view_reopens_both_filters_and_entry_drafts_disable_switching(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    let id = Uuid::from_u128(91);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |view, cx| {
            let Screen::Navigation(nav) = &mut view.screen else {
                unreachable!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                unreachable!()
            };
            table.views = vec![saved(id)];
            table.views_open = true;
            cx.notify();
        });
        w.render_frame(cx);
        w.click("add-one", cx);
        w.input("Keep draft", cx);
        w.render_frame(cx);
        assert_eq!(w.find("quick-filters").disabled(), Some(true));
        assert_eq!(w.find(format!("view-{id}")).disabled(), Some(true));
        assert_eq!(w.find("save-view").disabled(), Some(true));
        w.press("escape", cx);
        w.render_frame(cx);
        w.click(format!("view-{id}"), cx);
        assert_eq!(table(view.read(cx)).filter, "Sketch");
        assert_eq!(table(view.read(cx)).filters, saved(id).filters);
        assert_eq!(w.find("view-filter").value(), Some("Sketch"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn viewer_can_apply_saved_filters_but_cannot_change_or_save_them(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    let id = Uuid::from_u128(91);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |view, cx| {
            let Screen::Navigation(nav) = &mut view.screen else {
                unreachable!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                unreachable!()
            };
            table.editable = false;
            let mut shared = saved(id);
            shared.shared = true;
            table.views = vec![shared];
            table.views_open = true;
            cx.notify();
        });
        w.render_frame(cx);
        assert!(w.try_find("save-view").is_none());
        assert!(w.try_find("shared-view-heading").is_some());
        w.click(format!("view-{id}"), cx);
        assert_eq!(table(view.read(cx)).filters, saved(id).filters);
        w.click("quick-filters", cx);
        assert_eq!(w.find("filter-status-Done").disabled(), Some(true));
        assert_eq!(w.find("filter-group-one").disabled(), Some(true));
        assert_eq!(w.find("filter-clear").disabled(), Some(true));
        w.press("escape", cx);
        assert_eq!(w.find("quick-filters").focused(), Some(true));
    })
    .unwrap();
}

#[gpui_kit::test]
fn uncertain_save_retains_draft_and_reread_acknowledges_only_matching_visit(
    cx: &mut TestAppContext,
) {
    let (window, view) = visit(cx);
    let id = Uuid::from_u128(91);
    let board = Uuid::from_u128(8);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |view, cx| {
            let Screen::Navigation(nav) = &mut view.screen else {
                unreachable!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                unreachable!()
            };
            table.views_open = true;
            table
                .view_name
                .update(cx, |input, cx| input.set_value("Review", w, cx));
            table.view_save_id = Some(id);
            table.set_busy(true, cx);
            view.finish_view_save(
                view.navigation_request + 1,
                board,
                id,
                ViewSaveResult::Saved(saved(id)),
                w,
                cx,
            );
        });
        assert!(table(view.read(cx)).pending);
        assert!(table(view.read(cx)).views.is_empty());
        view.update(cx, |view, cx| {
            view.finish_view_save(
                view.navigation_request,
                board,
                id,
                ViewSaveResult::Checked(Err(EditError::Transport)),
                w,
                cx,
            )
        });
        w.render_frame(cx);
        assert_eq!(w.find("view-name").value(), Some("Review"));
        assert_eq!(w.find("save-view").disabled(), Some(true));
        assert!(w.try_find("check-saved-view").is_some());
        view.update(cx, |view, cx| {
            view.finish_view_save(
                view.navigation_request,
                board,
                id,
                ViewSaveResult::Checked(Ok(vec![saved(id)])),
                w,
                cx,
            )
        });
        w.render_frame(cx);
        assert_eq!(w.find("view-name").value(), Some(""));
        assert_eq!(table(view.read(cx)).views, [saved(id)]);
        assert!(table(view.read(cx)).view_save_id.is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn missing_status_is_not_confirmed_blank(cx: &mut TestAppContext) {
    let (window, view) = super::super::tests::visit(cx);
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |view, _| {
            let Screen::Navigation(nav) = &mut view.screen else {
                unreachable!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                unreachable!()
            };
            table.filters.include_empty_status = true;
        });
        assert!(table(view.read(cx)).order(cx).is_empty());
    })
    .unwrap();
}
