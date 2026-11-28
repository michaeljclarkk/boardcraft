use super::*;

impl StatusColumn {
    pub(in crate::navigation) fn selection_controls(
        &self,
        board: Uuid,
        visit: Uuid,
        busy: bool,
        window: &mut Window,
        cx: &mut Context<BoardView>,
    ) -> AnyElement {
        let disabled = busy || self.is_pending() || self.needs_reload || !self.editable;
        let trigger_focus = window
            .use_keyed_state("selection-status-column-focus", cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let column_focus = window
            .use_keyed_state("selection-status-option-focus", cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let placeholder_focus = window
            .use_keyed_state("selection-status-placeholder-focus", cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let trigger = Button::new("selection-status-column")
            .large()
            .label(if self.selection_column {
                self.title.clone()
            } else {
                "Change label…".into()
            })
            .accessibility_label("Bulk edit status column")
            .track_focus(&trigger_focus)
            .disabled(disabled);
        let mut controls = div().flex().flex_wrap().items_center().gap_2();
        if disabled {
            controls = controls.child(trigger);
        } else {
            let owner = cx.weak_entity();
            let title = self.title.clone();
            let column = self.id;
            let active_focus = if self.selection_column {
                &column_focus
            } else {
                &placeholder_focus
            };
            controls = controls.child(
                Popover::new("selection-status-columns")
                    .anchor(Anchor::TopRight)
                    .track_focus(active_focus)
                    .trigger(trigger.on_mouse_down(MouseButton::Left, move |_, w, cx| {
                        trigger_focus.focus(w, cx);
                    }))
                    .content(move |_, _, cx| {
                        let popup = cx.weak_entity();
                        let handles = [placeholder_focus.clone(), column_focus.clone()];
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children([false, true].into_iter().map(|selected| {
                                let owner = owner.clone();
                                let popup = popup.clone();
                                Button::new(if selected {
                                    "selection-status-column-option"
                                } else {
                                    "selection-status-column-empty"
                                })
                                .large()
                                .track_focus(if selected {
                                    &column_focus
                                } else {
                                    &placeholder_focus
                                })
                                .label(if selected {
                                    title.clone()
                                } else {
                                    "Change label…".into()
                                })
                                .on_click(move |_, w, cx| {
                                    _ = popup.update(cx, |popup, cx| popup.dismiss(w, cx));
                                    _ = owner.update(cx, |view, cx| {
                                        if view.navigation_visit != visit || view.editor.is_some() {
                                            return;
                                        }
                                        if let Some(Page::Selected {
                                            board: current,
                                            status,
                                            ..
                                        }) = &mut view.navigation
                                            && *current == board
                                            && status.id == column
                                            && status.editable
                                            && !status.is_pending()
                                            && !status.needs_reload
                                        {
                                            status.selection_column = selected;
                                            cx.notify();
                                        }
                                    });
                                })
                            }))
                            .on_key_down(move |event: &KeyDownEvent, w, cx| {
                                let next = match event.keystroke.key.as_str() {
                                    "down" | "up" => usize::from(handles[0].is_focused(w)),
                                    "home" => 0,
                                    "end" => 1,
                                    _ => return,
                                };
                                handles[next].focus(w, cx);
                                cx.stop_propagation();
                            })
                    }),
            );
        }
        if self.selection_column {
            let focus = window
                .use_keyed_state("selection-status-label-focus", cx, |_, cx| {
                    cx.focus_handle().tab_stop(true)
                })
                .read(cx)
                .clone();
            let trigger = Button::new("selection-status-label")
                .large()
                .label("Choose label")
                // Source prompt colour is distinct from the empty cell colour.
                .custom(tile_style(rgb(0x757575).into(), cx))
                .bg(rgb(0x757575))
                .text_color(gpui_kit::white())
                .accessibility_label(format!("{}: Choose label", self.title))
                .track_focus(&focus)
                .loading(self.selection_pending)
                .disabled(disabled);
            controls = if disabled
                && self.label_controls.picker_open.as_deref()
                    != Some(format!("selection-{board}").as_str())
            {
                controls.child(trigger)
            } else {
                controls.child(
                    Picker {
                        id: format!("selection-{board}"),
                        choices: self.choices.clone(),
                        current: None,
                        target: PickerTarget::Selection(SelectionTarget {
                            board,
                            column: self.id,
                            visit,
                        }),
                        trigger: trigger.on_mouse_down(MouseButton::Left, move |_, w, cx| {
                            focus.focus(w, cx);
                        }),
                        open: self.label_controls.picker_open.as_deref()
                            == Some(format!("selection-{board}").as_str()),
                        can_manage: self.label_controls.can_manage_labels,
                    }
                    .render(window, cx),
                )
            };
        }
        controls.into_any_element()
    }

    pub(in crate::navigation) fn selection_feedback(&self, cx: &App) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .children(
                self.selection_errors
                    .iter()
                    .enumerate()
                    .map(|(ix, (title, message))| {
                        Label::new(format!("{title}: {message}"))
                            .id(SharedString::from(format!("selection-status-error-{ix}")))
                            .text_color(cx.theme().danger)
                    }),
            )
            .into_any_element()
    }

    pub(in crate::navigation) fn clear_selection_state(&mut self) {
        self.selection_column = false;
    }
}

impl BoardView {
    pub(super) fn set_selected_status(
        &mut self,
        target: SelectionTarget,
        value: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let SelectionTarget {
            board,
            column,
            visit,
        } = target;
        if self.navigation_visit != visit
            || self.logging_out
            || self.editor.is_some()
            || self.filters_open
            || self.selected.is_empty()
        {
            return;
        }
        let (
            Some(connection),
            Some(client),
            Some(Page::Selected {
                board: current,
                status,
                rows,
                ..
            }),
        ) = (self.connection, self.client.clone(), &mut self.navigation)
        else {
            return;
        };
        if *current != board
            || status.id != column
            || !status.editable
            || !status.selection_column
            || status.is_pending()
            || status.needs_reload
            || value
                .as_ref()
                .is_some_and(|key| !status.choices.iter().any(|choice| choice.key == *key))
        {
            return;
        }
        status.selection_errors.clear();
        // Selection is independent of visible rows, collapsed groups and filters.
        let mut targets = Vec::new();
        for id in &self.selected {
            let title = rows
                .iter()
                .find(|row| row.id == *id)
                .map(|row| row.title.clone())
                .unwrap_or_else(|| "Unavailable item".into());
            match status.cells.get(id) {
                Some(cell) if cell.value != value => targets.push((*id, cell.revision, title)),
                Some(_) => {}
                None => status
                    .selection_errors
                    .push((title, "Status unavailable. Reload board.")),
            }
        }
        if targets.is_empty() {
            cx.notify();
            return;
        }
        targets.sort_unstable_by_key(|(id, _, _)| *id);
        status.selection_pending = true;
        self.message = "";
        let identifier = Login::identifier(window, cx);
        let work = cx.background_executor().spawn(async move {
            let mut accepted = Vec::new();
            let mut failures = Vec::new();
            let mut stopped = false;
            for (item, revision, title) in targets {
                if stopped {
                    failures.push((
                        title,
                        "Not saved after access changed. Review board access.",
                    ));
                    continue;
                }
                match client.set_status(board, column, item, revision, value.clone()) {
                    Ok(cell) => accepted.push(cell),
                    Err(EditError::SessionRejected) => return Err(EditError::SessionRejected),
                    Err(error) => {
                        stopped = matches!(error, EditError::PermissionDenied);
                        failures.push((
                            title,
                            match error {
                                EditError::Conflict => {
                                    "Status changed elsewhere. Review the current value."
                                }
                                EditError::PermissionDenied => "No permission to change status.",
                                EditError::Missing => "Item or column no longer available.",
                                EditError::InvalidDraft => {
                                    "Status choice unavailable. Choose again."
                                }
                                _ => "Could not confirm the change. Review the current value.",
                            },
                        ));
                    }
                }
            }
            let fresh = if failures.is_empty() {
                None
            } else {
                Some(StatusColumn::load(&client, board))
            };
            Ok((accepted, failures, fresh))
        });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            _ = view.update(cx, |view, cx| {
                if view.navigation_visit != visit { return; }
                let Some(Page::Selected { board: current, rows, groups, status, .. }) = &mut view.navigation else { return; };
                if *current != board || status.id != column || !status.selection_pending { return; }
                match result {
                    Err(EditError::SessionRejected) => view.forget_session(connection, identifier, cx),
                    Ok((accepted, failures, fresh)) => {
                        for cell in accepted { status.cells.insert(cell.item_id, cell); }
                        status.selection_pending = false;
                        status.selection_errors.extend(failures);
                        match fresh {
                            Some(Ok((mut fresh_rows, fresh_groups, mut fresh_status))) => {
                                for fresh in &mut fresh_rows {
                                    if let Some(current) = rows.iter().find(|row| row.id == fresh.id)
                                        && current.revision > fresh.revision
                                    { *fresh = current.clone(); }
                                }
                                fresh_status.selection_column = true;
                                fresh_status.selection_errors = std::mem::take(&mut status.selection_errors);
                                *rows = fresh_rows;
                                *groups = fresh_groups;
                                *status = fresh_status;
                                // Keep failed and hidden IDs selected so a partial save cannot
                                // silently change the scope of the user's next explicit choice.
                            }
                            Some(Err(EditError::SessionRejected)) => view.forget_session(connection, identifier, cx),
                            Some(Err(_)) => {
                                status.needs_reload = true;
                                view.message = "Some changes could not be confirmed. Reload board before changing status again.";
                            }
                            None => {},
                        }
                    }
                    Err(_) => unreachable!("Only session rejection stops selection saves"),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}
