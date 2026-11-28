use super::*;
use boardcraft_mobile_edit_client::{StatusCell, StatusChoice, ViewFilters};
use gpui_kit::component::{
    ActiveTheme, WindowExt,
    checkbox::Checkbox,
    tab::{Tab, TabBar},
};

#[derive(Clone)]
struct Filters {
    groups: HashSet<String>,
    statuses: HashSet<String>,
    blank: bool,
}

impl Filters {
    fn saved(&self) -> ViewFilters {
        let mut status_values: Vec<_> = self.statuses.iter().cloned().collect();
        let mut group_ids: Vec<_> = self.groups.iter().cloned().collect();
        status_values.sort();
        group_ids.sort();
        ViewFilters {
            status_values,
            include_empty_status: self.blank,
            group_ids,
        }
    }

    fn matches(
        &self,
        row: &SavedItem,
        groups: &[MainTableGroup],
        cells: &HashMap<Uuid, StatusCell>,
        query: &str,
    ) -> bool {
        view_filter_matches(query, &row.title)
            && self.in_group(row.id, groups)
            && if self.blank {
                cells.get(&row.id).is_some_and(|cell| cell.value.is_none())
            } else {
                self.statuses.is_empty()
                    || cells
                        .get(&row.id)
                        .and_then(|cell| cell.value.as_ref())
                        .is_some_and(|value| self.statuses.contains(value))
            }
    }

    fn in_group(&self, item: Uuid, groups: &[MainTableGroup]) -> bool {
        self.groups.is_empty()
            || groups.iter().any(|group| {
                self.groups.contains(group.id()) && group.items().iter().any(|row| row.id == item)
            })
    }
}

/// Kept by the board while a save is unconfirmed, including closing/reopening
/// Filters. Checking this exact identity never submits another create request.
#[derive(Clone)]
pub(crate) struct ViewDraft {
    id: Uuid,
    name: String,
    query: String,
    filters: ViewFilters,
    uncertain: bool,
}

impl ViewDraft {
    fn is_saved_as(&self, view: &SavedView) -> bool {
        view.id == self.id
            && view.name == self.name
            && view.filter == self.query
            && view.filters == self.filters
    }
}

/// One opened native view-settings dialog owns its draft and debounce. The board
/// receives preview values; closing keeps them, while Discard restores the baseline.
struct FilterDialog {
    owner: WeakEntity<BoardView>,
    board: Uuid,
    visit: Uuid,
    baseline: Filters,
    draft: Filters,
    groups: Vec<MainTableGroup>,
    rows: Vec<SavedItem>,
    choices: Vec<StatusChoice>,
    cells: HashMap<Uuid, StatusCell>,
    status_title: String,
    query: String,
    section: usize,
    writable: bool,
    dirty: bool,
    closed: bool,
    preview: Option<Task<()>>,
    name: Entity<InputState>,
    request: Option<ViewDraft>,
    saving: bool,
    message: &'static str,
}

impl BoardView {
    pub(super) fn matches_board_filter(
        &self,
        row: &SavedItem,
        groups: &[MainTableGroup],
        status: &StatusColumn,
        query: &str,
    ) -> bool {
        view_filter_matches(query, &row.title)
            && (self.filter_groups.is_empty()
                || groups.iter().any(|group| {
                    self.filter_groups.contains(group.id())
                        && group.items().iter().any(|item| item.id == row.id)
                }))
            && if self.filter_blank_status {
                status
                    .cells
                    .get(&row.id)
                    .is_some_and(|cell| cell.value.is_none())
            } else {
                self.filter_status.is_empty()
                    || status
                        .cells
                        .get(&row.id)
                        .and_then(|cell| cell.value.as_ref())
                        .is_some_and(|value| self.filter_status.contains(value))
            }
    }

    pub(crate) fn open_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.is_none()
            && self.search_pending
            && matches!(self.navigation, Some(Page::Selected { .. }))
        {
            self.search_task = None;
            self.apply_search(cx);
        }
        let Some(Page::Selected {
            board,
            groups,
            rows,
            status,
            filter,
            ..
        }) = &self.navigation
        else {
            return;
        };
        if self.filters_open
            || self.editor.is_some()
            || self.logging_out
            || self.automation_pending
            || status.is_pending()
        {
            return;
        }
        let baseline = Filters {
            groups: self.filter_groups.clone(),
            statuses: self.filter_status.clone(),
            blank: self.filter_blank_status,
        };
        let owner = cx.weak_entity();
        let dialog = cx.new(|_| FilterDialog {
            owner,
            board: *board,
            visit: self.navigation_visit,
            draft: baseline.clone(),
            baseline,
            groups: groups.clone(),
            rows: rows.clone(),
            choices: status
                .choices
                .iter()
                .filter(|choice| choice.value().is_some())
                .cloned()
                .collect(),
            cells: status.cells.clone(),
            status_title: status.title.clone(),
            query: filter.clone(),
            section: 0,
            writable: status.editable,
            dirty: false,
            closed: false,
            preview: None,
            name: self.view_name.clone(),
            request: self.view_draft.clone(),
            saving: false,
            message: if self
                .view_draft
                .as_ref()
                .is_some_and(|draft| draft.uncertain)
            {
                "The last save is unconfirmed. Check saved views before saving again."
            } else {
                ""
            },
        });
        self.filters_open = true;
        self.search_focused = false;
        self.filter_focus.focus(window, cx);
        window.open_dialog(cx, move |surface, window, cx| {
            let close = dialog.clone();
            let dismiss = dialog.clone();
            let actions = dialog.update(cx, |dialog, cx| dialog.footer(cx));
            let saving = dialog.read(cx).saving;
            surface
                .w(window.viewport_size().width)
                .centered()
                .close_button(false)
                .keyboard(!saving)
                .overlay_closable(!saving)
                .title(
                    div()
                        .flex()
                        .items_center()
                        .w_full()
                        .child(div().flex_1().child(Label::new("Filters")))
                        .child(
                            Button::new("mobile-filter-close")
                                .large()
                                .ghost()
                                .label("Close")
                                .disabled(saving)
                                .on_click(move |_, window, cx| {
                                    if close.update(cx, |dialog, cx| dialog.finish(false, cx)) {
                                        window.close_dialog(cx);
                                    }
                                }),
                        ),
                )
                .on_close(move |_, _, cx| {
                    dismiss.update(cx, |dialog, cx| dialog.finish(false, cx));
                })
                .child(dialog.clone())
                .footer(actions)
        });
        cx.notify();
    }
}

impl FilterDialog {
    fn locked(&self) -> bool {
        !self.writable || self.saving || self.request.as_ref().is_some_and(|r| r.uncertain)
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let checking = self.request.as_ref().is_some_and(|r| r.uncertain);
        if self.closed || self.saving || (!self.writable && !checking) {
            return;
        }
        let name = self.name.read(cx).value().trim().to_owned();
        if !checking && name.is_empty() {
            self.message = "Enter a view name.";
            cx.notify();
            return;
        }
        let filters = self.draft.saved();
        let mut request = if checking {
            self.request.clone().expect("unconfirmed request")
        } else {
            let id = self
                .request
                .as_ref()
                .filter(|r| r.name == name && r.query == self.query && r.filters == filters)
                .map_or_else(Uuid::new_v4, |r| r.id);
            ViewDraft {
                id,
                name,
                query: self.query.clone(),
                filters,
                uncertain: false,
            }
        };
        // Persist the uncertain identity before crossing the network boundary.
        request.uncertain = true;
        let context = self.owner.update(cx, |view, cx| {
            if view.navigation_visit != self.visit
                || view.editor.is_some()
                || view.logging_out
                || view.automation_pending
                || !matches!(&view.navigation, Some(Page::Selected { board, status, .. })
                    if *board == self.board && (checking || status.editable) && !status.is_pending())
            {
                return None;
            }
            let client = view.client.clone()?;
            let connection = view.connection?;
            view.view_draft = Some(request.clone());
            Some((client, connection, Login::identifier(window, cx)))
        });
        let Ok(Some((client, connection, identifier))) = context else {
            return;
        };
        self.preview = None;
        self.publish(&self.draft, cx);
        self.saving = true;
        self.message = "";
        self.request = Some(request.clone());
        let board = self.board;
        let work = cx.background_executor().spawn(async move {
            if checking {
                return client
                    .list_views(board)
                    .map_err(|error| (error, true))
                    .and_then(|views| {
                        let saved = views.iter().find(|v| v.id == request.id).cloned();
                        if saved
                            .as_ref()
                            .is_some_and(|view| !request.is_saved_as(view))
                        {
                            return Err((EditError::Conflict, false));
                        }
                        Ok((saved, Some(views)))
                    });
            }
            match client.create_view_with_filters(
                board,
                request.id,
                &request.name,
                &request.query,
                &request.filters,
            ) {
                Ok(saved) => Ok((Some(saved), None)),
                Err(
                    error @ (EditError::SessionRejected
                    | EditError::PermissionDenied
                    | EditError::InvalidDraft),
                ) => Err((error, false)),
                Err(error) => match client.list_views(board) {
                    Ok(views) => match views.iter().find(|v| v.id == request.id).cloned() {
                        Some(saved) if request.is_saved_as(&saved) => {
                            Ok((Some(saved), Some(views)))
                        }
                        Some(_) => Err((EditError::Conflict, false)),
                        None => Err((error, false)),
                    },
                    Err(read_error) => Err((read_error, true)),
                },
            }
        });
        cx.spawn_in(window, async move |dialog, cx| {
            let result = work.await;
            let _ = dialog.update_in(cx, |dialog, window, cx| {
                dialog.saving = false;
                if dialog.closed { return; }
                let current = dialog.owner.update(cx, |view, _| {
                    view.navigation_visit == dialog.visit
                        && matches!(view.navigation, Some(Page::Selected { board, .. }) if board == dialog.board)
                }).unwrap_or(false);
                if !current {
                    dialog.closed = true;
                    window.close_dialog(cx);
                    return;
                }
                match result {
                    Ok((saved, loaded)) => {
                        let accepted = saved.is_some();
                        let _ = dialog.owner.update(cx, |view, cx| {
                            if let Some(Page::Selected { views, .. }) = &mut view.navigation {
                                if let Some(loaded) = loaded { *views = loaded; }
                                if let Some(saved) = saved {
                                    views.retain(|v| v.id != saved.id);
                                    views.push(saved);
                                    views.sort_by_key(|v| (v.name.to_lowercase(), v.id));
                                }
                            }
                            if accepted {
                                view.view_draft = None;
                                view.views_open = true;
                                view.view_name.update(cx, |input, cx| input.set_value("", window, cx));
                            } else if let Some(request) = &mut view.view_draft {
                                request.uncertain = false;
                            }
                            cx.notify();
                        });
                        if accepted {
                            dialog.request = None;
                            dialog.message = "Personal view saved.";
                        } else {
                            if let Some(request) = &mut dialog.request { request.uncertain = false; }
                            dialog.message = "The view was not saved. Review it and choose Save personal view.";
                        }
                    }
                    Err((EditError::SessionRejected, _)) => {
                        let _ = dialog.owner.update(cx, |view, cx| view.forget_session(connection, identifier, cx));
                        dialog.closed = true;
                        window.close_dialog(cx);
                    }
                    Err((error, uncertain)) => {
                        if let Some(request) = &mut dialog.request { request.uncertain = uncertain; }
                        let _ = dialog.owner.update(cx, |view, _| view.view_draft = dialog.request.clone());
                        dialog.message = if uncertain {
                            "Could not confirm the save. Check saved views before saving again."
                        } else {
                            match error {
                                EditError::PermissionDenied => "No permission to save views.",
                                EditError::InvalidDraft => "Check the view name and filter choices, then save again.",
                                EditError::Conflict => "This view changed. Review the name and filters before saving again.",
                                _ => "Could not save the view. Your name and filters are kept. Try again.",
                            }
                        };
                    }
                }
                window.refresh();
                cx.notify();
            });
        }).detach();
        window.refresh();
        cx.notify();
    }

    fn publish(&self, filters: &Filters, cx: &mut Context<Self>) {
        let _ = self.owner.update(cx, |view, cx| {
            if !view.filters_open || view.navigation_visit != self.visit || !matches!(view.navigation, Some(Page::Selected { board, .. }) if board == self.board) {
                return;
            }
            view.filter_groups = filters.groups.clone();
            view.filter_status = filters.statuses.clone();
            view.filter_blank_status = filters.blank;
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

    fn finish(&mut self, discard: bool, cx: &mut Context<Self>) -> bool {
        if self.saving {
            return false;
        }
        if self.closed {
            return true;
        }
        self.preview = None;
        if discard {
            self.publish(&self.baseline, cx);
        } else if self.dirty {
            self.publish(&self.draft, cx);
        }
        self.closed = true;
        let _ = self.owner.update(cx, |view, cx| {
            if view.navigation_visit == self.visit {
                view.filters_open = false;
                cx.notify();
            }
        });
        true
    }

    fn footer(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .w_full()
            .gap_2()
            .child(
                Button::new("mobile-filter-discard")
                    .large()
                    .flex_1()
                    .label("Discard draft")
                    .disabled(self.saving)
                    .on_click(cx.listener(|dialog, _, window, cx| {
                        if dialog.finish(true, cx) {
                            window.close_dialog(cx);
                        }
                    })),
            )
            .child(
                Button::new("mobile-filter-apply")
                    .large()
                    .primary()
                    .flex_1()
                    .label("Apply")
                    .disabled(self.saving)
                    .on_click(cx.listener(|dialog, _, window, cx| {
                        if dialog.finish(false, cx) {
                            window.close_dialog(cx);
                        }
                    })),
            )
    }

    fn statuses(&self, cx: &mut Context<Self>) -> Div {
        let mut content = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new("All columns"))
            .child(Label::new(self.status_title.clone()));
        for choice in &self.choices {
            let key = choice.key.clone();
            let count = self
                .rows
                .iter()
                .filter(|row| {
                    view_filter_matches(&self.query, &row.title)
                        && self.draft.in_group(row.id, &self.groups)
                        && self.cells.get(&row.id).and_then(|cell| cell.value.as_ref())
                            == Some(&key)
                })
                .count();
            let label = if count > 0 {
                format!("{} {count}", choice.label)
            } else {
                choice.label.clone()
            };
            content = content.child(
                Button::new(SharedString::from(format!("mobile-filter-status-{key}")))
                    .large()
                    .w_full()
                    .disabled(self.locked())
                    .selected(self.draft.statuses.contains(&key))
                    .child(
                        div()
                            .size_3()
                            .rounded_full()
                            .bg(Hsla::parse_hex(&choice.color).unwrap_or(cx.theme().muted)),
                    )
                    .child(div().flex_1().text_left().child(label))
                    .on_click(cx.listener(move |dialog, _, _, cx| {
                        dialog.draft.blank = false;
                        if !dialog.draft.statuses.remove(&key) {
                            dialog.draft.statuses.insert(key.clone());
                        }
                        dialog.changed(cx);
                    })),
            );
        }
        content.child(
            Button::new("mobile-filter-status-blank")
                .large()
                .w_full()
                .disabled(self.locked())
                .selected(self.draft.blank)
                .child(div().flex_1().text_left().child("Blank"))
                .on_click(cx.listener(|dialog, _, _, cx| {
                    dialog.draft.blank = !dialog.draft.blank;
                    dialog.draft.statuses.clear();
                    dialog.changed(cx);
                })),
        )
    }

    fn groups(&self, cx: &mut Context<Self>) -> Div {
        let all = self.draft.groups.is_empty();
        let mut content = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new("Groups"))
            .child(
                Checkbox::new("mobile-filter-all-groups")
                    .large()
                    .min_h_11()
                    .label("All groups")
                    .checked(all)
                    .disabled(self.locked())
                    .on_click(cx.listener(|dialog, checked, _, cx| {
                        dialog.draft.groups = if *checked {
                            HashSet::new()
                        } else {
                            dialog
                                .groups
                                .iter()
                                .map(|group| group.id().to_owned())
                                .collect()
                        };
                        dialog.changed(cx);
                    })),
            );
        for group in &self.groups {
            let id = group.id().to_owned();
            content = content.child(
                Checkbox::new(SharedString::from(format!("mobile-filter-group-{id}")))
                    .large()
                    .min_h_11()
                    .label(group.name().to_owned())
                    .checked(all || self.draft.groups.contains(&id))
                    .disabled(
                        self.locked()
                            || (!all
                                && self.draft.groups.len() == 1
                                && self.draft.groups.contains(&id)),
                    )
                    .on_click(cx.listener(move |dialog, checked, _, cx| {
                        if *checked {
                            dialog.draft.groups.insert(id.clone());
                        } else {
                            if dialog.draft.groups.is_empty() {
                                dialog.draft.groups = dialog
                                    .groups
                                    .iter()
                                    .map(|group| group.id().to_owned())
                                    .collect();
                            }
                            dialog.draft.groups.remove(&id);
                        }
                        dialog.changed(cx);
                    })),
            );
        }
        content
    }
}

impl Render for FilterDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self
            .rows
            .iter()
            .filter(|row| {
                self.draft
                    .matches(row, &self.groups, &self.cells, &self.query)
            })
            .count();
        // Size the modal from the actual viewport, leaving room for its native
        // header/footer and outer safe margins rather than a fixed phone size.
        div()
            .flex()
            .flex_col()
            .gap_3()
            .min_h_0()
            .h(window.viewport_size().height * 0.68)
            .child(
                TabBar::new("mobile-filter-sections")
                    .underline()
                    .selected_index(self.section)
                    .child(Tab::new().label("Filter"))
                    .child(Tab::new().label("Groups"))
                    .on_click(cx.listener(|dialog, section, _, cx| {
                        dialog.section = *section;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().child(Label::new(if self.section == 0 {
                        "Quick filters"
                    } else {
                        "Groups"
                    })))
                    .child(
                        Button::new("mobile-filter-clear")
                            .large()
                            .label("Clear all")
                            .disabled(self.locked())
                            .on_click(cx.listener(|dialog, _, _, cx| {
                                if dialog.section == 0 {
                                    dialog.draft.statuses.clear();
                                    dialog.draft.blank = false;
                                } else {
                                    dialog.draft.groups.clear();
                                }
                                dialog.changed(cx);
                            })),
                    ),
            )
            .child(
                Label::new(format!("Showing {count} items"))
                    .text_color(cx.theme().muted_foreground),
            )
            .children(
                (!self.writable).then(|| Label::new("This account can read saved view settings.")),
            )
            .child(
                div()
                    .id("mobile-filter-options")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(if self.section == 0 {
                        self.statuses(cx)
                    } else {
                        self.groups(cx)
                    }),
            )
            .children(
                (self.writable || self.request.as_ref().is_some_and(|r| r.uncertain)).then(|| {
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(Label::new("Personal view name"))
                        .child(
                            Input::new(&self.name)
                                .id("mobile-view-name")
                                .large()
                                .aria_label("Personal view name")
                                .disabled(self.locked()),
                        )
                        .child(
                            Button::new("mobile-view-save")
                                .large()
                                .label(if self.saving {
                                    "Saving…"
                                } else if self.request.as_ref().is_some_and(|r| r.uncertain) {
                                    "Check saved views"
                                } else {
                                    "Save personal view"
                                })
                                .disabled(self.saving)
                                .on_click(
                                    cx.listener(|dialog, _, window, cx| dialog.save(window, cx)),
                                ),
                        )
                }),
            )
            .children((!self.message.is_empty()).then(|| Label::new(self.message)))
    }
}
