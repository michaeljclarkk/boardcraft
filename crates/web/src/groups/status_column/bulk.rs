//! Selection Status uses each captured cell revision and keeps independent failures.
use super::*;
use futures::StreamExt;

pub(super) struct BulkStatus {
    chosen: bool,
    chooser: FocusHandle,
    trigger: FocusHandle,
    initial: FocusHandle,
    baseline: Rc<RefCell<Option<SelectionTarget>>>,
    active: bool,
    batch: Uuid,
    pending: Vec<Uuid>,
    failures: Vec<Failure>,
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use crate::groups::tests::visit_titles;
    use gpui_kit::{
        TestAppContext,
        test::{TestEventExt, TestQueryExt, TestWindowExt},
    };

    fn accepted(value: Option<&str>, revision: i64) -> StatusTable {
        StatusTable {
            column_id: Uuid::from_u128(500),
            title: "Status".into(),
            choices: [
                ("working", "Working on it", "#fdab3d"),
                ("done", "Done", "#00c875"),
            ]
            .into_iter()
            .map(|(key, label, color)| StatusChoice {
                key: key.into(),
                label: label.into(),
                color: color.into(),
            })
            .collect(),
            cells: [59, 61]
                .into_iter()
                .map(|id| StatusCell {
                    item_id: Uuid::from_u128(id),
                    revision,
                    value: value.map(str::to_owned),
                })
                .collect(),
        }
    }

    #[gpui_kit::test]
    fn chooser_palette_keyboard_clear_noop_and_readonly(cx: &mut TestAppContext) {
        let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
        cx.update(|cx| cx.set_reduce_motion(true));
        cx.update_window(handle, |_, w, cx| {
            table.update(cx, |table, cx| table.status.accept(accepted(None, 1), cx));
            w.render_frame(cx);
            w.click(format!("check-{}", Uuid::from_u128(59)), cx);
            let bar = w.find("board-selection-bar").bounds();
            let chooser = w.find("bulk-status-column").bounds();
            assert_eq!(chooser.size.height, bar.size.height - gpui_kit::px(2.));
            assert_eq!(chooser.top(), bar.top() + gpui_kit::px(1.));
            assert_eq!(chooser.bottom(), bar.bottom() - gpui_kit::px(1.));
            assert!(w.try_find("bulk-status-label").is_none());
            w.click("bulk-status-column", cx);
            let status = w.find_by_label("Status").bounds().center();
            w.pointer_down(status, MouseButton::Left, cx);
            w.pointer_up(status, MouseButton::Left, cx);
            assert!(table.read(cx).status.bulk.chosen);
            assert_eq!(w.find("bulk-status-label").label(), Some("Choose label"));
            w.click("bulk-status-label", cx);
            assert_eq!(w.find("bulk-status-choice-working").focused(), Some(true));
            assert_ne!(
                w.find("bulk-status-empty").checked(),
                Some(true),
                "a bulk prompt has no selected label"
            );
            w.press("down", cx);
            assert_eq!(w.find("bulk-status-choice-done").focused(), Some(true));
            w.press("escape", cx);
            assert_eq!(w.find("bulk-status-label").focused(), Some(true));
            w.click("bulk-status-label", cx);
            w.click("bulk-status-clear", cx);
            assert!(!table.read(cx).pending);
            assert!(
                table.read(cx).status.request.is_none(),
                "clearing already-empty cells is a no-op"
            );
            assert_eq!(table.read(cx).selection, [Uuid::from_u128(59)]);
            w.click("clear-selected", cx);
            assert!(!table.read(cx).status.bulk.chosen);
        })
        .unwrap();
        let (handle, table) = visit_titles(cx, false, ["Ink", "Sketch"]);
        cx.update_window(handle, |_, w, cx| {
            table.update(cx, |table, cx| table.status.accept(accepted(None, 1), cx));
            w.render_frame(cx);
            w.click(format!("check-{}", Uuid::from_u128(59)), cx);
            assert_eq!(w.find("bulk-status-column").disabled(), Some(true));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn pending_bulk_cells_keep_table_geometry_and_independent_progress(cx: &mut TestAppContext) {
        let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
        cx.update_window(handle, |_, w, cx| {
            table.update(cx, |table, cx| {
                table.status.accept(accepted(None, 1), cx);
                table.selection = vec![Uuid::from_u128(59), Uuid::from_u128(61)];
                table.status.bulk.chosen = true;
            });
            w.render_frame(cx);
            let scroll = w.find("content-scroll").bounds();
            let row = w.find(format!("row-{}", Uuid::from_u128(59))).bounds();
            let (batch, targets, generation) = table.update(cx, |table, cx| {
                let capture = table.capture_selection_status().unwrap();
                let (batch, targets) = table
                    .begin_selection_status(&capture, &Some("done".into()), cx)
                    .unwrap();
                (batch, targets, capture.generation)
            });
            w.render_frame(cx);
            assert_eq!(w.find("content-scroll").bounds(), scroll);
            assert_eq!(w.find(format!("row-{}", Uuid::from_u128(59))).bounds(), row);
            assert_eq!(w.find("bulk-status-label").label(), Some("Saving…"));
            assert!(w.try_find("status-notice").is_none());
            assert!(table.read(cx).status.is_saving());
            table.update(cx, |table, cx| {
                table.finish_selection_status_item(
                    batch,
                    targets[0],
                    Some("done".into()),
                    Ok(StatusCell {
                        item_id: targets[0].item,
                        revision: 2,
                        value: Some("done".into()),
                    }),
                    cx,
                );
            });
            w.render_frame(cx);
            assert!(
                !w.find(format!("status-{}", targets[0].item))
                    .label()
                    .unwrap()
                    .contains("Saving")
            );
            assert!(
                w.find(format!("status-{}", targets[1].item))
                    .label()
                    .unwrap()
                    .contains("Saving")
            );
            table.update(cx, |table, cx| {
                table.finish_selection_status_item(
                    batch,
                    targets[1],
                    Some("done".into()),
                    Ok(StatusCell {
                        item_id: targets[1].item,
                        revision: 2,
                        value: Some("done".into()),
                    }),
                    cx,
                );
                table.finish_selection_status(batch, generation, None, cx);
            });
            w.render_frame(cx);
            assert_eq!(w.find("bulk-status-label").label(), Some("Choose label"));
            assert_eq!(table.read(cx).selection.len(), 2);
            assert!(table.read(cx).status.bulk.failures.is_empty());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn hidden_selection_uses_captured_revisions_and_partial_success_retains_failure(
        cx: &mut TestAppContext,
    ) {
        let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
        cx.update_window(handle, |_, w, cx| {
            table.update(cx, |table, cx| table.status.accept(accepted(None, 1), cx));
            w.render_frame(cx);
            w.click(format!("check-{}", Uuid::from_u128(59)), cx);
            w.click(format!("check-{}", Uuid::from_u128(61)), cx);
            w.click("add-input-g-b", cx);
            w.input("Draft stays", cx);
            let original_rows = table
                .read(cx)
                .rows
                .iter()
                .map(|(id, row)| (*id, row.entity_id()))
                .collect::<HashMap<_, _>>();
            table.update(cx, |table, cx| {
                table.result_search = "ink".into();
                table.view_filters.status_values = vec!["working".into()];
                let captured = table.capture_selection_status().unwrap();
                assert_eq!(captured.cells.len(), 2);
                table.status.accept(accepted(Some("working"), 2), cx);
                let (batch, changed) = table
                    .begin_selection_status(&captured, &Some("done".into()), cx)
                    .unwrap();
                assert_eq!(
                    changed
                        .iter()
                        .map(|target| target.revision)
                        .collect::<Vec<_>>(),
                    [1, 1]
                );
                table.finish_selection_status_item(
                    batch,
                    changed[0],
                    Some("done".into()),
                    Ok(StatusCell {
                        item_id: changed[0].item,
                        revision: 3,
                        value: Some("done".into()),
                    }),
                    cx,
                );
                assert!(!table.status.bulk.is_pending(changed[0].item));
                assert!(table.status.bulk.is_pending(changed[1].item));
                table.finish_selection_status_item(
                    batch,
                    changed[1],
                    Some("done".into()),
                    Err(EditError::Conflict),
                    cx,
                );
                table.finish_selection_status(
                    batch,
                    captured.generation,
                    Some(Ok(accepted(Some("working"), 2))),
                    cx,
                );
                assert_eq!(
                    table.status.accepted.as_ref().unwrap().cells[0]
                        .value
                        .as_deref(),
                    Some("done")
                );
                assert_eq!(table.status.bulk.failures.len(), 1);
                assert_eq!(table.status.bulk.failures[0].item, Uuid::from_u128(61));
                assert!(table.status.bulk.failures[0].reviewed);
                assert!(!table.status.failed);
            });
            w.render_frame(cx);
            assert!(
                w.try_find(format!("item-{}", Uuid::from_u128(61)))
                    .is_none()
            );
            assert_eq!(table.read(cx).selection.len(), 2);
            assert_eq!(
                table.read(cx).entries["g-b"].input.read(cx).value(),
                "Draft stays"
            );
            assert!(
                table
                    .read(cx)
                    .rows
                    .iter()
                    .all(|(id, row)| original_rows[id] == row.entity_id())
            );
            assert_ne!(
                w.find(format!("bulk-status-retry-{}", Uuid::from_u128(61)))
                    .disabled(),
                Some(true)
            );
            assert!(
                table.read(cx).status.request.is_none(),
                "recovery does not replay writes"
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn failed_recovery_blocks_retry_until_read_and_stale_completions_do_not_apply(
        cx: &mut TestAppContext,
    ) {
        let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
        let item = Uuid::from_u128(59);
        cx.update_window(handle, |_, w, cx| {
            table.update(cx, |table, cx| {
                table.status.accept(accepted(None, 1), cx);
                table.selection = vec![item];
                let captured = table.capture_selection_status().unwrap();
                let (batch, changed) = table
                    .begin_selection_status(&captured, &Some("done".into()), cx)
                    .unwrap();
                table.finish_selection_status_item(
                    Uuid::new_v4(),
                    changed[0],
                    Some("done".into()),
                    Ok(StatusCell {
                        item_id: item,
                        revision: 9,
                        value: Some("done".into()),
                    }),
                    cx,
                );
                assert_eq!(table.status.accepted.as_ref().unwrap().cells[0].revision, 1);
                table.finish_selection_status_item(
                    batch,
                    changed[0],
                    Some("done".into()),
                    Err(EditError::Transport),
                    cx,
                );
                table.finish_selection_status(
                    batch,
                    captured.generation,
                    Some(Err(EditError::Transport)),
                    cx,
                );
                assert!(table.status.failed);
                assert!(!table.status.bulk.failures[0].reviewed);
            });
            w.render_frame(cx);
            assert_eq!(
                w.find(format!("bulk-status-retry-{item}")).disabled(),
                Some(true)
            );
            assert!(w.find("reload-status").visible());
            table.update(cx, |table, cx| {
                table.finish_status_read(
                    table.status.generation,
                    Ok(accepted(Some("working"), 2)),
                    cx,
                );
                assert!(table.status.bulk.failures[0].reviewed);
                assert!(table.status.request.is_none());
                let mut stale = table.capture_selection_status().unwrap();
                stale.generation = Uuid::new_v4();
                assert!(table.begin_selection_status(&stale, &None, cx).is_none());
            });
            w.render_frame(cx);
            assert_ne!(
                w.find(format!("bulk-status-retry-{item}")).disabled(),
                Some(true)
            );
        })
        .unwrap();
    }
}

#[derive(Clone)]
struct SelectionTarget {
    generation: Uuid,
    column: Uuid,
    cells: Vec<StatusCell>,
}

struct Failure {
    item: Uuid,
    name: String,
    value: Option<String>,
    label: String,
    error: EditError,
    reviewed: bool,
}

impl BulkStatus {
    pub(super) fn new(cx: &mut App) -> Self {
        Self {
            chosen: false,
            chooser: cx.focus_handle().tab_stop(true),
            trigger: cx.focus_handle().tab_stop(true),
            initial: cx.focus_handle().tab_stop(true),
            baseline: Rc::new(RefCell::new(None)),
            active: false,
            batch: Uuid::new_v4(),
            pending: Vec::new(),
            failures: Vec::new(),
        }
    }

    pub(super) fn active(&self) -> bool {
        self.active
    }
    pub(super) fn is_pending(&self, item: Uuid) -> bool {
        self.pending.contains(&item)
    }
    pub(super) fn notice(&self) -> Option<String> {
        (!self.failures.is_empty()).then(|| {
            format!(
                "{} Status changes need review. Selection is kept.",
                self.failures.len()
            )
        })
    }
    pub(super) fn review(&mut self, read: &StatusTable) {
        for failure in &mut self.failures {
            failure.reviewed = read.cells.iter().any(|cell| cell.item_id == failure.item);
        }
    }
    pub(super) fn clear_failure(&mut self, item: Uuid) {
        self.failures.retain(|failure| failure.item != item);
    }
}

impl Table {
    pub(in crate::groups) fn reset_bulk_status_choice(&mut self) {
        self.status.bulk.chosen = false;
        self.status.bulk.baseline.replace(None);
    }

    fn capture_selection_status(&self) -> Option<SelectionTarget> {
        let accepted = self.status.accepted.as_ref()?;
        let cells: Option<Vec<_>> = self
            .selection
            .iter()
            .map(|id| {
                accepted
                    .cells
                    .iter()
                    .find(|cell| cell.item_id == *id)
                    .cloned()
            })
            .collect();
        Some(SelectionTarget {
            generation: self.status.generation,
            column: accepted.column_id,
            cells: cells?,
        })
    }

    pub(in crate::groups) fn bulk_status_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.busy()
            || !self.read.is_editable()
            || self.status.loading
            || self.status.failed
            || self.status.accepted.is_none();
        let view = cx.weak_entity();
        let title = self
            .status
            .accepted
            .as_ref()
            .map_or("Status", |table| table.title.as_str())
            .to_owned();
        let choose = view.clone();
        let reset = view.clone();
        let chooser = Button::new("bulk-status-column")
            .label(if self.status.bulk.chosen {
                title.clone()
            } else {
                "Change label…".into()
            })
            .accessibility_label("Bulk edit status column")
            .track_focus(&self.status.bulk.chooser)
            .icon(IconName::ChevronDown)
            .small()
            .w(rems(9.5))
            // DropdownMenu's Popover wrapper is intrinsically sized; a percent
            // height resolves against its text, not the selection bar. Fill the
            // shared 70px bar's inner height explicitly (one border pixel each side).
            .h(rems(4.25))
            .rounded_none()
            .disabled(disabled)
            .dropdown_menu(move |menu, _, _| {
                let reset = reset.clone();
                let choose = choose.clone();
                menu.item(
                    PopupMenuItem::new("Change label…").on_click(move |_, _, cx| {
                        let _ = reset.update(cx, |table, cx| {
                            table.reset_bulk_status_choice();
                            cx.notify();
                        });
                    }),
                )
                .item(PopupMenuItem::new(title.clone()).on_click(move |_, w, cx| {
                    let _ = choose.update(cx, |table, cx| {
                        if table.busy() || !table.read.is_editable() {
                            return;
                        }
                        table.status.bulk.chosen = true;
                        table.status.bulk.trigger.focus(w, cx);
                        cx.notify();
                    });
                }))
            });
        let mut controls = div()
            .flex()
            .items_start()
            .h_full()
            .flex_shrink_0()
            .child(chooser);
        if !self.status.bulk.chosen {
            return controls.into_any_element();
        }
        let prompt = Button::new("bulk-status-label")
            .label(if self.status.bulk.active {
                "Saving…"
            } else {
                "Choose label"
            })
            .track_focus(&self.status.bulk.trigger)
            .w(rems(7.5))
            .h(rems(2.125))
            .rounded_none()
            .custom(tile_style(gpui_kit::rgb(0x757575).into(), cx))
            .bg(gpui_kit::rgb(0x757575))
            .text_color(gpui_kit::white());
        let Some(target) = self.capture_selection_status() else {
            return controls.child(prompt.disabled(true)).into_any_element();
        };
        if disabled {
            return controls.child(prompt.disabled(true)).into_any_element();
        }
        let choices = self.status.accepted.as_ref().unwrap().choices.clone();
        let baseline = self.status.bulk.baseline.clone();
        let capture = baseline.clone();
        let initial = self.status.bulk.initial.clone();
        let trigger = self.status.bulk.trigger.clone();
        controls = controls.child(
            Popover::new("bulk-status-popup")
                .track_focus(&initial)
                .on_open_change(move |open, _, _| {
                    if *open {
                        capture.replace(Some(target.clone()));
                    }
                })
                .trigger(
                    prompt.on_mouse_down(MouseButton::Left, move |_, w, cx| trigger.focus(w, cx)),
                )
                .content(move |_, w, cx| {
                    let popup = cx.weak_entity();
                    let commit = |value: Option<String>| {
                        let view = view.clone();
                        let popup = popup.clone();
                        let baseline = baseline.clone();
                        move |_: &ClickEvent, w: &mut Window, cx: &mut App| {
                            let target = baseline.borrow().clone();
                            let _ = popup
                                .update(cx, |popup: &mut PopoverState, cx| popup.dismiss(w, cx));
                            if let Some(target) = target {
                                let _ = view.update(cx, |table, cx| {
                                    table.set_selection_status(target, value.clone(), w, cx)
                                });
                            }
                        }
                    };
                    let mut options = div()
                        .id("bulk-status-options")
                        .role(Role::ListBox)
                        .aria_label("Choose Status")
                        .flex()
                        .flex_col()
                        .gap_2();
                    let mut row = div().flex().gap_2();
                    let mut handles = Vec::new();
                    let options_count = choices.len()
                        + usize::from(!choices.iter().any(|choice| choice.key == "empty"));
                    for (ix, option) in choices
                        .iter()
                        .map(Some)
                        .chain(
                            (!choices.iter().any(|choice| choice.key == "empty")).then_some(None),
                        )
                        .enumerate()
                    {
                        let id = key(option.map_or_else(
                            || "bulk-status-empty".into(),
                            |choice| {
                                if choice.key == "empty" {
                                    "bulk-status-empty".into()
                                } else {
                                    format!("bulk-status-choice-{}", choice.key)
                                }
                            },
                        ));
                        let handle = if ix == 0 {
                            initial.clone()
                        } else {
                            w.use_keyed_state(id.clone(), cx, |_, cx| {
                                cx.focus_handle().tab_stop(true)
                            })
                            .read(cx)
                            .clone()
                        };
                        let bg = color(option, cx);
                        row = row.child(
                            div().flex_1().min_w_0().flex_basis(rems(0.)).child(
                                Button::new(id)
                                    .role(Role::ListBoxOption)
                                    .track_focus(&handle)
                                    .w_full()
                                    .min_w_0()
                                    .h_8()
                                    .label(
                                        option
                                            .filter(|choice| !choice.label.is_empty())
                                            .map_or("Empty", |choice| choice.label.as_str())
                                            .to_owned(),
                                    )
                                    .custom(tile_style(bg, cx))
                                    .bg(bg)
                                    .text_color(gpui_kit::white())
                                    .on_click(commit(option.and_then(StatusChoice::value))),
                            ),
                        );
                        handles.push(handle);
                        if ix % 2 == 1 || ix + 1 == options_count {
                            if ix % 2 == 0 {
                                row = row.child(div().flex_1().flex_basis(rems(0.)).min_w_0());
                            }
                            options = options.child(row);
                            row = div().flex().gap_2();
                        }
                    }
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .w(rems(21.))
                        .child(options)
                        .child(
                            Button::new("bulk-status-clear")
                                .w_full()
                                .label("Clear value")
                                .on_click(commit(None)),
                        )
                        .on_key_down(move |event: &KeyDownEvent, w, cx| {
                            let Some(ix) = handles.iter().position(|handle| handle.is_focused(w))
                            else {
                                return;
                            };
                            let count = handles.len();
                            let next = match event.keystroke.key.as_str() {
                                "right" | "down" => (ix + 1) % count,
                                "left" | "up" => (ix + count - 1) % count,
                                "home" => 0,
                                "end" => count - 1,
                                _ => return,
                            };
                            handles[next].focus(w, cx);
                            cx.stop_propagation();
                        })
                }),
        );
        controls.into_any_element()
    }

    fn begin_selection_status(
        &mut self,
        target: &SelectionTarget,
        value: &Option<String>,
        cx: &mut Context<Self>,
    ) -> Option<(Uuid, Vec<Target>)> {
        if target.generation != self.status.generation
            || self.busy()
            || !self.read.is_editable()
            || self.status.loading
            || self.status.failed
        {
            return None;
        }
        let accepted = self.status.accepted.as_ref()?;
        if accepted.column_id != target.column
            || value
                .as_ref()
                .is_some_and(|key| !accepted.choices.iter().any(|choice| &choice.key == key))
        {
            return None;
        }
        let changed: Vec<_> = target
            .cells
            .iter()
            .filter(|cell| cell.value != *value)
            .map(|cell| Target {
                generation: target.generation,
                column: target.column,
                item: cell.item_id,
                revision: cell.revision,
            })
            .collect();
        for cell in &target.cells {
            self.status.bulk.clear_failure(cell.item_id);
        }
        self.status.notice = self.status.bulk.notice();
        if changed.is_empty() {
            cx.notify();
            return None;
        }
        self.status.bulk.active = true;
        self.status.bulk.batch = Uuid::new_v4();
        self.status.bulk.pending = changed.iter().map(|target| target.item).collect();
        self.pending = true;
        cx.notify();
        Some((self.status.bulk.batch, changed))
    }

    fn set_selection_status(
        &mut self,
        target: SelectionTarget,
        value: Option<String>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.status.bulk.trigger.focus(w, cx);
        let Some((batch, changed)) = self.begin_selection_status(&target, &value, cx) else {
            return;
        };
        let binding = self.binding.clone();
        let board = self.board;
        let executor = cx.background_executor().clone();
        self.status.request = Some(cx.spawn_in(w, async move |table, cx| {
            let mut requests = futures::stream::iter(changed.into_iter().map(|target| {
                let binding = binding.clone();
                let executor = executor.clone();
                let value = value.clone();
                async move {
                    let result = session::set_status(
                        &binding,
                        board,
                        target.column,
                        target.item,
                        &StatusEdit {
                            revision: target.revision,
                            value,
                        },
                        &executor,
                    )
                    .await;
                    (target, result)
                }
            }))
            .buffer_unordered(4);
            let mut failed = false;
            let mut denied = false;
            while let Some((item, result)) = requests.next().await {
                failed |= result.is_err();
                denied |= matches!(
                    result,
                    Err(EditError::SessionRejected | EditError::PermissionDenied)
                );
                let _ = table.update(cx, |table, cx| {
                    table.finish_selection_status_item(batch, item, value.clone(), result, cx)
                });
            }
            let reread = if failed && !denied {
                Some(session::read_status(&binding, board, &executor).await)
            } else {
                None
            };
            let _ = table.update(cx, |table, cx| {
                table.finish_selection_status(batch, target.generation, reread, cx)
            });
        }));
    }

    fn finish_selection_status_item(
        &mut self,
        batch: Uuid,
        target: Target,
        value: Option<String>,
        result: Result<StatusCell, EditError>,
        cx: &mut Context<Self>,
    ) {
        if self.status.generation != target.generation
            || self.status.bulk.batch != batch
            || !self.status.bulk.active
        {
            return;
        }
        self.status.bulk.pending.retain(|id| *id != target.item);
        match result {
            Ok(saved) => {
                if let Some(accepted) = &mut self.status.accepted
                    && let Some(cell) = accepted
                        .cells
                        .iter_mut()
                        .find(|cell| cell.item_id == target.item)
                    && saved.item_id == target.item
                    && saved.revision >= cell.revision
                {
                    *cell = saved;
                }
            }
            Err(error) => {
                if matches!(
                    error,
                    EditError::SessionRejected | EditError::PermissionDenied
                ) {
                    self.status.failed = true;
                    cx.emit(AccessLost);
                }
                let name = self.rows.get(&target.item).map_or_else(
                    || "Unavailable item".into(),
                    |row| row.read(cx).accepted_title().to_owned(),
                );
                let label = self
                    .status
                    .accepted
                    .as_ref()
                    .and_then(|table| {
                        table
                            .choices
                            .iter()
                            .find(|choice| Some(&choice.key) == value.as_ref())
                    })
                    .map_or("Empty", |choice| choice.label.as_str())
                    .to_owned();
                self.status.bulk.failures.push(Failure {
                    item: target.item,
                    name,
                    value,
                    label,
                    error,
                    reviewed: false,
                });
            }
        }
        cx.notify();
    }

    fn finish_selection_status(
        &mut self,
        batch: Uuid,
        generation: Uuid,
        reread: Option<Result<StatusTable, EditError>>,
        cx: &mut Context<Self>,
    ) {
        if self.status.generation != generation
            || self.status.bulk.batch != batch
            || !self.status.bulk.active
        {
            return;
        }
        self.pending = false;
        self.status.bulk.active = false;
        self.status.bulk.pending.clear();
        match reread {
            Some(Ok(mut read)) => {
                // A stale recovery snapshot must not undo an acknowledged cell write.
                if let Some(accepted) = &self.status.accepted {
                    for cell in &mut read.cells {
                        if let Some(newer) = accepted
                            .cells
                            .iter()
                            .find(|old| old.item_id == cell.item_id && old.revision > cell.revision)
                        {
                            *cell = newer.clone();
                        }
                    }
                }
                self.status.bulk.review(&read);
                self.status.accept(read, cx);
            }
            Some(Err(error)) => {
                self.status.failed = true;
                if matches!(
                    error,
                    EditError::SessionRejected | EditError::PermissionDenied
                ) {
                    cx.emit(AccessLost);
                }
            }
            None if self
                .status
                .bulk
                .failures
                .iter()
                .any(|failure| !failure.reviewed) =>
            {
                self.status.failed = true;
            }
            None => {}
        }
        self.status.notice = self.status.bulk.notice();
        cx.notify();
    }

    fn retry_selection_status(&mut self, item: Uuid, w: &mut Window, cx: &mut Context<Self>) {
        let Some(value) = self
            .status
            .bulk
            .failures
            .iter()
            .find(|failure| failure.item == item && failure.reviewed)
            .map(|failure| failure.value.clone())
        else {
            return;
        };
        let Some(accepted) = &self.status.accepted else {
            return;
        };
        let Some(cell) = accepted
            .cells
            .iter()
            .find(|cell| cell.item_id == item)
            .cloned()
        else {
            return;
        };
        let target = SelectionTarget {
            generation: self.status.generation,
            column: accepted.column_id,
            cells: vec![cell],
        };
        self.set_selection_status(target, value, w, cx);
    }

    pub(super) fn bulk_status_failures(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.status
            .bulk
            .failures
            .iter()
            .map(|failure| {
                let item = failure.item;
                let reason = if failure.error == EditError::Conflict {
                    "Status changed elsewhere"
                } else {
                    "Status save not confirmed"
                };
                div()
                    .id(key(format!("bulk-status-failure-{item}")))
                    .role(Role::Status)
                    .test_support()
                    .w_full()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(format!(
                        "{}: {reason}. Review the current value before retrying {}.",
                        failure.name, failure.label
                    ))
                    .child(
                        Button::new(key(format!("bulk-status-retry-{item}")))
                            .small()
                            .label(format!("Retry {} for {}", failure.label, failure.name))
                            .disabled(
                                self.busy()
                                    || !self.read.is_editable()
                                    || self.status.loading
                                    || self.status.failed
                                    || !failure.reviewed,
                            )
                            .on_click(cx.listener(move |table, _, w, cx| {
                                table.retry_selection_status(item, w, cx)
                            })),
                    )
                    .into_any_element()
            })
            .collect()
    }
}
