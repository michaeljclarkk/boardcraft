use super::*;
use crate::connected::main_table::tests::table;
use core::prelude::v1::test;
use gpui_kit::test::TestWindowExt;

fn snapshot() -> StatusTable {
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
        cells: vec![
            StatusCell {
                item_id: Uuid::from_u128(42),
                revision: 2,
                value: Some("Done".into()),
            },
            StatusCell {
                item_id: Uuid::from_u128(17),
                revision: 4,
                value: None,
            },
        ],
    }
}

fn visit(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Connected>) {
    let (window, view) = crate::connected::main_table::tests::visit(cx);
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
            table.status.accept(snapshot(), cx);
            table.selection.ids = vec![Uuid::from_u128(42)];
            cx.notify();
        });
    });
    (window, view)
}

#[gpui_kit::test]
fn column_then_palette_restores_focus_and_unchanged_choice_does_not_write(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("selection-status-picker").is_none());
        w.click("selection-status-column", cx);
        assert_eq!(
            w.find("selection-status-column").expanded(),
            Some(true),
            "selector {:?}, input {:?}, disabled {:?}, pending {}, ready {}",
            w.find("selection-status-column").bounds(),
            w.within("selection-status-column").find("input").bounds(),
            w.find("selection-status-column").disabled(),
            table(view.read(cx)).pending,
            table(view.read(cx)).status.filters_ready()
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.press("down", cx);
        w.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        assert_eq!(
            table(view.read(cx))
                .status
                .bulk
                .column
                .read(cx)
                .selected_value()
                .map(|s| s.as_ref()),
            Some("Status")
        );
        w.render_frame(cx);
        let picker = w.find("selection-status-picker").bounds();
        assert_eq!(picker.size.width, rems(7.5).to_pixels(w.rem_size()));
        assert_eq!(picker.size.height, rems(2.125).to_pixels(w.rem_size()));
        assert_eq!(
            picker.top(),
            w.find("selection-status-column").bounds().top()
        );
        w.click("selection-status-picker", cx);
        let captured = table(view.read(cx)).status.bulk.palette.clone().unwrap();
        view.update(cx, |view, cx| {
            let Screen::Navigation(nav) = &mut view.screen else {
                unreachable!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                unreachable!()
            };
            if let ColumnRead::Ready { cells, .. } = &mut table.status.read {
                cells.get_mut(&Uuid::from_u128(42)).unwrap().saved.revision = 9;
            }
            cx.notify();
        });
        w.render_frame(cx);
        assert_eq!(
            table(view.read(cx))
                .status
                .bulk
                .palette
                .as_ref()
                .unwrap()
                .cells,
            captured.cells
        );
        assert_eq!(
            w.find("selection-status-choice-Working").focused(),
            Some(true)
        );
        let first = w.find("selection-status-choice-Working").bounds();
        let second = w.find("selection-status-choice-Done").bounds();
        assert_eq!(first.top(), second.top());
        assert!(second.left() > first.left());
        assert_ne!(w.find("selection-status-choice-Done").checked(), Some(true));
        w.press("escape", cx);
        assert_eq!(w.find("selection-status-picker").focused(), Some(true));
        w.click("selection-status-picker", cx);
        w.click("selection-status-choice-Done", cx);
        assert!(!table(view.read(cx)).pending);
        assert!(table(view.read(cx)).status.bulk.pending.is_empty());
        assert_eq!(table(view.read(cx)).selection.ids, [Uuid::from_u128(42)]);
        assert_eq!(w.find("selection-status-picker").focused(), Some(true));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(table(view.read(cx)).status.bulk.failures.is_empty()));
}

#[gpui_kit::test]
fn readonly_and_item_drafts_disable_bulk_controls(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    cx.update_window(window, |_, w, cx| {
        w.render_frame(cx);
        w.click("add-one", cx);
        w.input("Keep my draft", cx);
        w.render_frame(cx);
        assert_eq!(w.find("selection-status-column").disabled(), Some(true));
        w.press("escape", cx);
        view.update(cx, |view, cx| {
            let Screen::Navigation(nav) = &mut view.screen else {
                unreachable!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                unreachable!()
            };
            table.editable = false;
            cx.notify();
        });
        w.render_frame(cx);
        assert_eq!(w.find("selection-status-column").disabled(), Some(true));
    })
    .unwrap();
}

#[gpui_kit::test]
fn partial_success_keeps_hidden_selection_and_reconciliation_does_not_replay(
    cx: &mut TestAppContext,
) {
    let (window, view) = visit(cx);
    let done = Uuid::from_u128(42);
    let failed = Uuid::from_u128(17);
    let request = Uuid::new_v4();
    cx.update_window(window, |_, w, cx| {
        let visit = view.update(cx, |view, cx| {
            let Screen::Navigation(nav) = &mut view.screen else {
                unreachable!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                unreachable!()
            };
            table.selection.ids = vec![done, failed];
            table.filter = "Sketch".into();
            table.status.bulk.request = Some(request);
            table.status.bulk.pending = vec![done, failed];
            table.status.bulk.choice = "Working".into();
            table.set_busy(true, cx);
            table.status.visit
        });
        view.update(cx, |view, cx| {
            view.finish_bulk_cell(
                Uuid::new_v4(),
                request,
                done,
                Ok(StatusCell {
                    item_id: done,
                    revision: 3,
                    value: Some("Working".into()),
                }),
                w,
                cx,
            )
        });
        assert_eq!(table(view.read(cx)).status.bulk.pending, [done, failed]);
        view.update(cx, |view, cx| {
            view.finish_bulk_cell(
                visit,
                request,
                done,
                Ok(StatusCell {
                    item_id: done,
                    revision: 3,
                    value: Some("Working".into()),
                }),
                w,
                cx,
            )
        });
        assert_eq!(
            table(view.read(cx)).status.filter_value(done),
            Some(Some("Working"))
        );
        assert_eq!(table(view.read(cx)).status.bulk.pending, [failed]);
        view.update(cx, |view, cx| {
            view.finish_bulk_cell(visit, request, failed, Err(EditError::Conflict), w, cx)
        });
        let review = table(view.read(cx)).status.bulk.request.unwrap();
        let mut read = snapshot();
        read.cells[0].value = Some("Working".into());
        read.cells[0].revision = 3;
        read.cells[1].value = Some("Stuck".into());
        read.cells[1].revision = 5;
        view.update(cx, |view, cx| {
            view.finish_bulk_review(visit, review, Ok(read), w, cx)
        });
        w.render_frame(cx);
        let current = table(view.read(cx));
        assert!(!current.pending);
        assert_eq!(current.selection.ids, [done, failed]);
        assert_eq!(current.order(cx), [done]);
        assert_eq!(current.status.filter_value(failed), Some(Some("Stuck")));
        assert!(
            current
                .status
                .bulk
                .failures
                .get(&failed)
                .unwrap()
                .contains("changed elsewhere")
        );
        assert!(
            w.try_find(format!("selection-status-failure-{failed}"))
                .is_some()
        );
        assert!(current.status.bulk.request.is_none());
        assert!(
            w.find("selection-status-review").bounds().bottom()
                < w.find("board-selection-bar").bounds().top()
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let current = table(view.read(cx));
        assert_eq!(current.status.filter_value(failed), Some(Some("Stuck")));
        assert!(current.status.bulk.pending.is_empty());
    });
}

#[gpui_kit::test]
fn unavailable_reconciliation_keeps_failure_and_disables_replay(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    let request = Uuid::new_v4();
    let item = Uuid::from_u128(42);
    cx.update_window(window, |_, w, cx| {
        let visit = view.update(cx, |view, cx| {
            let Screen::Navigation(nav) = &mut view.screen else {
                unreachable!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                unreachable!()
            };
            table.status.bulk.request = Some(request);
            table
                .status
                .bulk
                .failures
                .insert(item, "Status not confirmed".into());
            table.set_busy(true, cx);
            table.status.visit
        });
        view.update(cx, |view, cx| {
            view.finish_bulk_review(visit, request, Err(EditError::Transport), w, cx)
        });
        w.render_frame(cx);
        assert!(table(view.read(cx)).status.requires_reload);
        assert_eq!(w.find("selection-status-column").disabled(), Some(true));
        assert_ne!(w.find("selection-status-review").disabled(), Some(true));
        assert!(
            table(view.read(cx))
                .status
                .bulk
                .failures
                .contains_key(&item)
        );
        assert_eq!(table(view.read(cx)).selection.ids, [item]);
    })
    .unwrap();
}

#[gpui_kit::test]
fn permission_denial_stays_readonly_after_status_reread(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    let request = Uuid::new_v4();
    let item = Uuid::from_u128(42);
    cx.update_window(window, |_, w, cx| {
        let visit = view.update(cx, |view, cx| {
            let Screen::Navigation(nav) = &mut view.screen else {
                unreachable!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                unreachable!()
            };
            table.status.bulk.request = Some(request);
            table.status.bulk.pending = vec![item];
            table.set_busy(true, cx);
            table.status.visit
        });
        view.update(cx, |view, cx| {
            view.finish_bulk_cell(
                visit,
                request,
                item,
                Err(EditError::PermissionDenied),
                w,
                cx,
            )
        });
        let review = table(view.read(cx)).status.bulk.request.unwrap();
        view.update(cx, |view, cx| {
            view.finish_bulk_review(visit, review, Ok(snapshot()), w, cx)
        });
        w.render_frame(cx);
        assert!(!table(view.read(cx)).editable);
        assert_eq!(w.find("selection-status-column").disabled(), Some(true));
        assert_eq!(table(view.read(cx)).selection.ids, [item]);
        assert_eq!(
            table(view.read(cx)).status.filter_value(item),
            Some(Some("Done"))
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn two_stale_cells_render_both_failures_without_duplicate_identity(cx: &mut TestAppContext) {
    let (window, view) = visit(cx);
    let request = Uuid::new_v4();
    let first = Uuid::from_u128(42);
    let second = Uuid::from_u128(17);
    cx.update_window(window, |_, w, cx| {
        let visit = view.update(cx, |view, cx| {
            let Screen::Navigation(nav) = &mut view.screen else {
                unreachable!()
            };
            let Page::Items(_, table) = &mut nav.page else {
                unreachable!()
            };
            table.selection.ids = vec![first, second];
            table.status.bulk.request = Some(request);
            table.status.bulk.pending = vec![first, second];
            table.status.bulk.choice = "Stuck".into();
            table.set_busy(true, cx);
            table.status.visit
        });
        view.update(cx, |view, cx| {
            view.finish_bulk_cell(visit, request, first, Err(EditError::Conflict), w, cx)
        });
        view.update(cx, |view, cx| {
            view.finish_bulk_cell(visit, request, second, Err(EditError::Conflict), w, cx)
        });
        w.render_frame(cx);
        assert!(
            w.try_find(format!("selection-status-failure-{first}"))
                .is_some()
        );
        assert!(
            w.try_find(format!("selection-status-failure-{second}"))
                .is_some()
        );
        assert!(w.try_find(format!("status-notice-{first}")).is_some());
        assert!(w.try_find(format!("status-notice-{second}")).is_some());
        assert_eq!(table(view.read(cx)).selection.ids, [first, second]);
    })
    .unwrap();
}
