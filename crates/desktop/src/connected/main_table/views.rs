//! Quick-filter drafts preview locally; only an acknowledged save adds a personal view.
use super::*;
use boardcraft_mobile_edit_client::{StatusChoice, ViewFilters};
use gpui_kit::component::{Colorize, WindowExt};
use std::time::Duration;

struct FilterDialog {
    owner: WeakEntity<Connected>,
    board: Uuid,
    visit: u64,
    baseline: ViewFilters,
    draft: ViewFilters,
    groups: Vec<(String, String, String)>,
    choices: Vec<StatusChoice>,
    rows: Vec<(String, String, Option<Option<String>>)>,
    query: String,
    writable: bool,
    dirty: bool,
    closed: bool,
    preview: Option<Task<()>>,
}

impl MainTable {
    pub(super) fn has_filters(&self) -> bool {
        !self.filter.is_empty() || self.filters != ViewFilters::default()
    }

    pub(super) fn view_actions_blocked(&self, cx: &App) -> bool {
        self.pending
            || self.reconcile
            || self.filters_open
            || self.view_save_id.is_some()
            || self.headings.iter().any(|h| {
                h.entry.read(cx).has_draft(cx)
                    || h.rows.iter().any(|r| r.read(cx).baseline.is_some())
            })
    }

    pub(super) fn render_views(&self, cx: &mut Context<Connected>) -> impl IntoElement {
        let blocked = self.view_actions_blocked(cx);
        let mut panel = div()
            .id("views")
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new("Views"));
        if self.editable {
            panel = panel.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Input::new(&self.view_name)
                            .id("view-name")
                            .disabled(self.pending || self.view_save_id.is_some()),
                    )
                    .child(
                        Button::new("save-view")
                            .small()
                            .label("Save view")
                            .disabled(blocked || !self.editable)
                            .on_click(cx.listener(|view, _, w, cx| view.save_view(w, cx))),
                    ),
            );
        }
        if self.view_save_unconfirmed {
            panel = panel.child(
                Button::new("check-saved-view")
                    .small()
                    .label("Check saved view")
                    .disabled(self.pending)
                    .on_click(cx.listener(|view, _, w, cx| view.check_saved_view(w, cx))),
            );
        }
        for shared in [false, true] {
            if self.views.iter().any(|view| view.shared == shared) {
                panel = panel.child(
                    Label::new(if shared {
                        "Shared views"
                    } else {
                        "Personal views"
                    })
                    .id(if shared {
                        "shared-view-heading"
                    } else {
                        "personal-view-heading"
                    }),
                );
            }
            for view in self.views.iter().filter(|view| view.shared == shared) {
                let id = view.id;
                panel = panel.child(
                    Button::new(SharedString::from(format!("view-{id}")))
                        .small()
                        .label(view.name.clone())
                        .disabled(blocked)
                        .on_click(
                            cx.listener(move |view, _, w, cx| view.apply_saved_view(id, w, cx)),
                        ),
                );
            }
        }
        panel
    }
}

impl Connected {
    pub(super) fn clear_all_view_filters(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Screen::Navigation(nav) = &mut self.screen
            && let Page::Items(_, table) = &mut nav.page
        {
            if table.editable {
                table.filters = ViewFilters::default();
            }
        }
        self.clear_view_filter(w, cx);
    }

    pub(super) fn open_quick_filters(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        let Page::Items(board, table) = &nav.page else {
            return;
        };
        if table.view_actions_blocked(cx) || !table.status.filters_ready() {
            return;
        }
        let state = FilterDialog {
            owner: cx.weak_entity(),
            board: *board,
            visit: self.navigation_request,
            baseline: table.filters.clone(),
            draft: table.filters.clone(),
            groups: table
                .headings
                .iter()
                .map(|h| (h.id.to_string(), h.name.to_string(), h.colour.to_string()))
                .collect(),
            choices: table.status.filter_choices(),
            rows: table
                .headings
                .iter()
                .flat_map(|h| {
                    h.rows.iter().map(|row| {
                        let row = row.read(cx);
                        (
                            h.id.to_string(),
                            row.edit.accepted().to_owned(),
                            table
                                .status
                                .filter_value(row.edit.identity().1.as_uuid())
                                .map(|value| value.map(str::to_owned)),
                        )
                    })
                })
                .collect(),
            query: table.filter.clone(),
            writable: table.editable,
            dirty: false,
            closed: false,
            preview: None,
        };
        let dialog = cx.new(|_| state);
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(_, table) = &mut nav.page else {
            return;
        };
        table.filters_open = true;
        table.filter_focus.focus(w, cx);
        w.open_dialog(cx, move |surface, w, cx| {
            let close = dialog.clone();
            let dismiss = dialog.clone();
            let footer = dialog.update(cx, |dialog, cx| dialog.footer(cx));
            surface
                .w(rems(30.).to_pixels(w.rem_size()))
                .max_w(w.viewport_size().width - rems(2.).to_pixels(w.rem_size()))
                .close_button(false)
                .title(
                    div()
                        .flex()
                        .items_center()
                        .w_full()
                        .child(div().flex_1().child("Quick filters"))
                        .child(
                            Button::new("filter-close")
                                .ghost()
                                .small()
                                .label("Close")
                                .on_click(move |_, w, cx| {
                                    close.update(cx, |dialog, cx| dialog.finish(false, cx));
                                    w.close_dialog(cx);
                                }),
                        ),
                )
                .on_close(move |_, _, cx| {
                    dismiss.update(cx, |dialog, cx| dialog.finish(false, cx));
                })
                .child(dialog.clone())
                .footer(footer)
        });
        cx.notify();
    }

    fn apply_saved_view(&mut self, id: Uuid, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(_, table) = &mut nav.page else {
            return;
        };
        if table.view_actions_blocked(cx) {
            return;
        }
        let Some(saved) = table.views.iter().find(|view| view.id == id).cloned() else {
            return;
        };
        table.filter = saved.filter.clone();
        table.filters = saved.filters;
        table.search_task = None;
        table.search_invalid = false;
        table.selection_cache = false;
        table.drag.borrow_mut().cancel();
        for h in &table.headings {
            h.row_drag.borrow_mut().cancel();
        }
        table
            .view_filter
            .update(cx, |input, cx| input.set_value(saved.filter, w, cx));
        cx.notify();
    }

    pub(super) fn save_view(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, table) = &mut nav.page else {
            return;
        };
        if table.view_actions_blocked(cx) || !table.editable {
            return;
        }
        let name = table.view_name.read(cx).value().trim().to_owned();
        let query = table.view_filter.read(cx).value().trim().to_owned();
        if name.is_empty()
            || name.encode_utf16().count() > 160
            || query.encode_utf16().count() > 240
        {
            self.message = "Enter a view name and a valid search.";
            cx.notify();
            return;
        }
        let id = Uuid::new_v4();
        let board = *board;
        let filters = table.filters.clone();
        let client = nav.client.clone();
        let visit = self.navigation_request;
        table.view_save_id = Some(id);
        table.view_save_unconfirmed = false;
        table.set_busy(true, cx);
        self.message = "Saving view…";
        let work = cx.background_executor().spawn(async move {
            match client.create_view_with_filters(board, id, &name, &query, &filters) {
                Ok(saved) => ViewSaveResult::Saved(saved),
                Err(
                    error @ (EditError::InvalidDraft
                    | EditError::Conflict
                    | EditError::PermissionDenied
                    | EditError::SessionRejected),
                ) => ViewSaveResult::Rejected(error),
                Err(_) => ViewSaveResult::Checked(client.list_views(board)),
            }
        });
        cx.spawn_in(w, async move |view, cx| {
            let result = work.await;
            let _ = view.update_in(cx, |view, w, cx| {
                view.finish_view_save(visit, board, id, result, w, cx)
            });
        })
        .detach();
        cx.notify();
    }

    fn check_saved_view(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, table) = &mut nav.page else {
            return;
        };
        let Some(id) = table.view_save_id else {
            return;
        };
        if table.pending || !table.view_save_unconfirmed {
            return;
        }
        let board = *board;
        let visit = self.navigation_request;
        let client = nav.client.clone();
        table.set_busy(true, cx);
        self.message = "Checking saved views…";
        let work = cx
            .background_executor()
            .spawn(async move { client.list_views(board) });
        cx.spawn_in(w, async move |view, cx| {
            let result = ViewSaveResult::Checked(work.await);
            let _ = view.update_in(cx, |view, w, cx| {
                view.finish_view_save(visit, board, id, result, w, cx)
            });
        })
        .detach();
        cx.notify();
    }

    fn finish_view_save(
        &mut self,
        visit: u64,
        board: Uuid,
        id: Uuid,
        result: ViewSaveResult,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.navigation_request != visit {
            return;
        }
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(current, table) = &mut nav.page else {
            return;
        };
        if *current != board || table.view_save_id != Some(id) {
            return;
        }
        table.set_busy(false, cx);
        let result = match result {
            ViewSaveResult::Checked(Ok(views)) => {
                let saved = views.iter().find(|view| view.id == id).cloned();
                table.views = views;
                saved.map(ViewSaveResult::Saved)
            }
            other => Some(other),
        };
        table.view_save_id = None;
        table.view_save_unconfirmed = false;
        match result {
            Some(ViewSaveResult::Saved(saved)) => {
                table.views.retain(|view| view.id != saved.id);
                table.views.push(saved);
                table.views.sort_by_key(|view| view.name.to_lowercase());
                table
                    .view_name
                    .update(cx, |input, cx| input.set_value("", w, cx));
                self.message = "View saved";
            }
            Some(
                ViewSaveResult::Rejected(EditError::SessionRejected)
                | ViewSaveResult::Checked(Err(EditError::SessionRejected)),
            ) => {
                self.link_input = None;
                self.screen = Self::identifier(w, cx);
                self.message = "Session expired. Sign in again.";
            }
            Some(ViewSaveResult::Rejected(EditError::Conflict)) => {
                self.message = "A view with that name already exists."
            }
            Some(ViewSaveResult::Rejected(EditError::PermissionDenied)) => {
                self.message = "View not saved: permission denied."
            }
            Some(ViewSaveResult::Rejected(_)) => {
                self.message = "Could not save the view. Review its name and filters."
            }
            Some(ViewSaveResult::Checked(Err(error))) => {
                table.view_save_id = Some(id);
                table.view_save_unconfirmed = true;
                self.message = if error == EditError::PermissionDenied {
                    "View save not confirmed: permission to check saved views was denied."
                } else {
                    "View save not confirmed. Check saved view before trying again."
                };
            }
            None => {
                self.message =
                    "View was not saved. Your draft is retained; choose Save view to try again."
            }
            Some(ViewSaveResult::Checked(Ok(_))) => unreachable!(),
        }
        cx.notify();
    }
}

enum ViewSaveResult {
    Saved(SavedView),
    Rejected(EditError),
    Checked(Result<Vec<SavedView>, EditError>),
}

impl FilterDialog {
    fn facet_count(&self, value: Option<&str>) -> usize {
        self.rows
            .iter()
            .filter(|(group, title, status)| {
                boardcraft_mobile_edit_client::view_filter_matches(&self.query, title)
                    && (self.draft.group_ids.is_empty() || self.draft.group_ids.contains(group))
                    && status
                        .as_ref()
                        .is_some_and(|status| status.as_deref() == value)
            })
            .count()
    }

    fn publish(&self, filters: &ViewFilters, cx: &mut Context<Self>) {
        let _ = self.owner.update(cx, |view, cx| {
            if view.navigation_request != self.visit {
                return;
            }
            let Screen::Navigation(nav) = &mut view.screen else {
                return;
            };
            let Page::Items(board, table) = &mut nav.page else {
                return;
            };
            if *board != self.board || !table.filters_open {
                return;
            }
            table.filters = filters.clone();
            table.selection_cache = false;
            table.drag.borrow_mut().cancel();
            for h in &table.headings {
                h.row_drag.borrow_mut().cancel();
            }
            cx.notify();
        });
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.dirty = true;
        self.preview = Some(cx.spawn(async move |dialog, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(200))
                .await;
            let _ = dialog.update(cx, |dialog, cx| {
                if !dialog.closed {
                    dialog.publish(&dialog.draft, cx);
                }
            });
        }));
        cx.notify();
    }
    fn finish(&mut self, discard: bool, cx: &mut Context<Self>) {
        if self.closed {
            return;
        }
        self.preview = None;
        if discard {
            self.publish(&self.baseline, cx);
        } else if self.dirty {
            self.publish(&self.draft, cx);
        }
        self.closed = true;
        let _ = self.owner.update(cx, |view, cx| {
            if view.navigation_request == self.visit
                && let Screen::Navigation(nav) = &mut view.screen
                && let Page::Items(board, table) = &mut nav.page
                && *board == self.board
            {
                table.filters_open = false;
                cx.notify();
            }
        });
    }
    fn footer(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .justify_end()
            .gap_2()
            .child(
                Button::new("filter-discard")
                    .label("Discard draft")
                    .on_click(cx.listener(|dialog, _, w, cx| {
                        dialog.finish(true, cx);
                        w.close_dialog(cx);
                    })),
            )
            .child(
                Button::new("filter-apply")
                    .primary()
                    .label("Apply")
                    .disabled(!self.writable)
                    .on_click(cx.listener(|dialog, _, w, cx| {
                        dialog.finish(false, cx);
                        w.close_dialog(cx);
                    })),
            )
    }
}

impl Render for FilterDialog {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self
            .rows
            .iter()
            .filter(|(group, title, status)| {
                boardcraft_mobile_edit_client::view_filter_matches(&self.query, title)
                    && ((self.draft.status_values.is_empty() && !self.draft.include_empty_status)
                        || status.is_some())
                    && self
                        .draft
                        .matches(status.as_ref().and_then(|value| value.as_deref()), group)
            })
            .count();
        let mut body = div()
            .id("filter-options")
            .flex()
            .flex_col()
            .gap_2()
            .max_h(w.viewport_size().height * 0.6)
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Label::new(format!("Showing {count} items")).id("filter-match-count"))
                    .child(
                        Button::new("filter-clear")
                            .ghost()
                            .small()
                            .label("Clear all")
                            .disabled(!self.writable)
                            .on_click(cx.listener(|dialog, _, _, cx| {
                                dialog.draft = ViewFilters::default();
                                dialog.changed(cx);
                            })),
                    ),
            )
            .child(Label::new("All columns"))
            .child(Label::new("Status"));
        for choice in &self.choices {
            let key = choice.key.clone();
            let selected = self.draft.status_values.contains(&key);
            let count = self.facet_count(Some(&key));
            body = body.child(
                Button::new(SharedString::from(format!("filter-status-{key}")))
                    .small()
                    .w(rems(9.375))
                    .h_8()
                    .min_h_8()
                    .flex_none()
                    .selected(selected)
                    .toggled(selected)
                    .disabled(!self.writable)
                    .accessibility_label(choice.label.clone())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .w_full()
                            .child(
                                div()
                                    .size(rems(0.625))
                                    .flex_none()
                                    .rounded_full()
                                    .bg(Hsla::parse_hex(&choice.color)
                                        .unwrap_or(cx.theme().muted_foreground)),
                            )
                            .child(div().flex_1().child(choice.label.clone()))
                            .child(if count == 0 {
                                String::new()
                            } else {
                                count.to_string()
                            }),
                    )
                    .on_click(cx.listener(move |dialog, _, _, cx| {
                        dialog.draft.include_empty_status = false;
                        dialog.draft.status_values.retain(|value| value != &key);
                        if !selected {
                            dialog.draft.status_values.push(key.clone());
                        }
                        dialog.changed(cx);
                    })),
            );
        }
        let count = self.facet_count(None);
        body = body
            .child(
                Button::new("filter-status-empty")
                    .accessibility_label("Blank")
                    .small()
                    .w(rems(9.375))
                    .h_8()
                    .min_h_8()
                    .flex_none()
                    .selected(self.draft.include_empty_status)
                    .toggled(self.draft.include_empty_status)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .w_full()
                            .child(div().flex_1().child("Blank"))
                            .child(if count == 0 {
                                String::new()
                            } else {
                                count.to_string()
                            }),
                    )
                    .disabled(!self.writable)
                    .on_click(cx.listener(|dialog, _, _, cx| {
                        dialog.draft.include_empty_status = !dialog.draft.include_empty_status;
                        dialog.draft.status_values.clear();
                        dialog.changed(cx);
                    })),
            )
            .child(Label::new("Groups"))
            .child(
                Checkbox::new("filter-all-groups")
                    .label("All groups")
                    .checked(self.draft.group_ids.is_empty())
                    .disabled(!self.writable)
                    .on_change(cx.listener(|dialog, checked, _, cx| {
                        dialog.draft.group_ids = if *checked {
                            Vec::new()
                        } else {
                            dialog.groups.iter().map(|(id, _, _)| id.clone()).collect()
                        };
                        dialog.changed(cx);
                    })),
            );
        for (id, name, colour) in &self.groups {
            let id = id.clone();
            body =
                body.child(
                    Checkbox::new(SharedString::from(format!("filter-group-{id}")))
                        .accessibility_label(name.clone())
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().size(rems(0.625)).flex_none().rounded_full().bg(
                                    Hsla::parse_hex(colour).unwrap_or(cx.theme().muted_foreground),
                                ))
                                .child(name.clone()),
                        )
                        .checked(
                            self.draft.group_ids.is_empty() || self.draft.group_ids.contains(&id),
                        )
                        .disabled(
                            !self.writable
                                || (self.draft.group_ids.len() == 1
                                    && self.draft.group_ids.contains(&id)),
                        )
                        .on_change(cx.listener(move |dialog, checked: &bool, _, cx| {
                            if !*checked && dialog.draft.group_ids.is_empty() {
                                dialog.draft.group_ids =
                                    dialog.groups.iter().map(|(id, _, _)| id.clone()).collect();
                            }
                            dialog.draft.group_ids.retain(|group| group != &id);
                            if *checked {
                                dialog.draft.group_ids.push(id.clone());
                            }
                            dialog.changed(cx);
                        })),
                );
        }
        for id in self
            .draft
            .group_ids
            .iter()
            .filter(|id| !self.groups.iter().any(|(current, _, _)| current == *id))
        {
            let id = id.clone();
            body = body.child(
                Checkbox::new(SharedString::from(format!("filter-group-{id}")))
                    .label(format!("Unavailable group ({id})"))
                    .checked(true)
                    .disabled(!self.writable)
                    .on_change(cx.listener(move |dialog, _, _, cx| {
                        dialog.draft.group_ids.retain(|group| group != &id);
                        dialog.changed(cx);
                    })),
            );
        }
        body
    }
}

#[cfg(all(test, feature = "native-smoke"))]
mod tests;
