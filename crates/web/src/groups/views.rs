//! Quick-filter drafts preview into this board visit; saved presets remain host-owned.
use super::*;
use boardcraft_mobile_edit_client::{SavedView, StatusTable};
use gpui_kit::{
    Hsla,
    base::{Selectable, StyledExt},
    component::{Colorize, button::ButtonVariants},
};

pub(super) struct FilterDialog {
    owner: WeakEntity<Table>,
    baseline: ViewFilters,
    draft: ViewFilters,
    writable: bool,
    groups: Vec<(String, String)>,
    rows: Vec<(Uuid, String, String)>,
    status: Option<StatusTable>,
    closed: bool,
    preview: Option<Task<()>>,
}

impl FilterDialog {
    fn publish(&self, filters: ViewFilters, cx: &mut Context<Self>) {
        let id = cx.entity_id();
        let _ = self.owner.update(cx, |table, cx| {
            if table
                .filter_dialog
                .as_ref()
                .is_some_and(|d| d.entity_id() == id)
            {
                table.view_filters = filters;
                table.drag.borrow_mut().cancel();
                for drag in table.row_drags.values() {
                    drag.borrow_mut().cancel();
                }
                cx.notify();
            }
        });
    }

    fn changed(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.preview = Some(cx.spawn_in(w, async move |dialog, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(200))
                .await;
            let _ = dialog.update_in(cx, |dialog, _, cx| {
                if !dialog.closed {
                    dialog.publish(dialog.draft.clone(), cx);
                }
            });
        }));
        cx.notify();
    }

    fn toggle_status(&mut self, value: Option<String>, w: &mut Window, cx: &mut Context<Self>) {
        if !self.writable || self.closed {
            return;
        }
        match value {
            None => {
                self.draft.include_empty_status = !self.draft.include_empty_status;
                self.draft.status_values.clear();
            }
            Some(value) => {
                self.draft.include_empty_status = false;
                if self.draft.status_values.contains(&value) {
                    self.draft.status_values.retain(|key| key != &value);
                } else {
                    self.draft.status_values.push(value);
                }
            }
        }
        self.changed(w, cx);
    }

    fn toggle_group(&mut self, value: String, w: &mut Window, cx: &mut Context<Self>) {
        if !self.writable || self.closed {
            return;
        }
        if self.draft.group_ids.is_empty() {
            self.draft.group_ids = self
                .groups
                .iter()
                .map(|g| g.0.clone())
                .filter(|id| id != &value)
                .collect();
        } else if self.draft.group_ids.contains(&value) {
            if self.draft.group_ids.len() == 1 {
                return;
            }
            self.draft.group_ids.retain(|id| id != &value);
        } else {
            self.draft.group_ids.push(value);
        }
        self.changed(w, cx);
    }

    fn finish(&mut self, discard: bool, cx: &mut Context<Self>) {
        if self.closed {
            return;
        }
        self.preview = None;
        self.publish(
            if discard {
                self.baseline.clone()
            } else {
                self.draft.clone()
            },
            cx,
        );
        self.closed = true;
        let id = cx.entity_id();
        let _ = self.owner.update(cx, |table, cx| {
            if table
                .filter_dialog
                .as_ref()
                .is_some_and(|d| d.entity_id() == id)
            {
                table.filter_dialog = None;
                cx.notify();
            }
        });
    }

    fn count(&self, status: Option<&str>) -> usize {
        let Some(table) = &self.status else {
            return 0;
        };
        self.rows
            .iter()
            .filter(|(id, group, _)| {
                (self.draft.group_ids.is_empty() || self.draft.group_ids.contains(group))
                    && table
                        .cells
                        .iter()
                        .any(|cell| cell.item_id == *id && cell.value.as_deref() == status)
            })
            .count()
    }
}

impl Render for FilterDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .capture_key_down(
                cx.listener(|dialog, event: &gpui_kit::KeyDownEvent, w, cx| {
                    if event.keystroke.key == "escape" {
                        dialog.finish(false, cx);
                        w.close_dialog(cx);
                        cx.stop_propagation();
                    }
                }),
            );
        if !self.writable {
            body = body.child(status("View settings are read-only for your role."));
        }
        body = body.child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(div().font_semibold().child("Status"))
                .child(
                    Button::new("filter-clear")
                        .label("Clear all")
                        .ghost()
                        .disabled(!self.writable)
                        .on_click(cx.listener(|dialog, _, w, cx| {
                            dialog.draft = ViewFilters::default();
                            dialog.changed(w, cx);
                        })),
                ),
        );
        if let Some(table) = &self.status {
            let mut choices = div().flex().flex_col().gap_1p5().w(gpui_kit::rems(9.375));
            for choice in table.choices.iter().filter(|choice| choice.key != "empty") {
                let value = choice.key.clone();
                let color = Hsla::parse_hex(&choice.color).unwrap_or(cx.theme().muted_foreground);
                let count = self.count(Some(&choice.key));
                choices = choices.child(
                    Button::new(key(format!("filter-status-{}", choice.key)))
                        .accessibility_label(format!("{} ({count})", choice.label))
                        .small()
                        .w_full()
                        .min_h_8()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .w_full()
                                .child(div().size_2p5().rounded_full().bg(color))
                                .child(div().flex_1().child(choice.label.clone()))
                                .child(div().text_color(cx.theme().muted_foreground).child(
                                    if count == 0 {
                                        String::new()
                                    } else {
                                        count.to_string()
                                    },
                                )),
                        )
                        .selected(self.draft.status_values.contains(&choice.key))
                        .toggled(self.draft.status_values.contains(&choice.key))
                        .disabled(!self.writable)
                        .on_click(cx.listener(move |dialog, _, w, cx| {
                            dialog.toggle_status(Some(value.clone()), w, cx)
                        })),
                );
            }
            let count = self.count(None);
            choices = choices.child(
                Button::new("filter-status-blank")
                    .accessibility_label(format!("Blank ({count})"))
                    .small()
                    .w_full()
                    .min_h_8()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .w_full()
                            .child(div().flex_1().child("Blank"))
                            .child(div().text_color(cx.theme().muted_foreground).child(
                                if count == 0 {
                                    String::new()
                                } else {
                                    count.to_string()
                                },
                            )),
                    )
                    .selected(self.draft.include_empty_status)
                    .toggled(self.draft.include_empty_status)
                    .disabled(!self.writable)
                    .on_click(cx.listener(|dialog, _, w, cx| dialog.toggle_status(None, w, cx))),
            );
            body = body.child(choices);
        } else {
            body = body.child(status(
                "Status choices are unavailable. Close and reload statuses to filter them.",
            ));
        }
        body = body.child(div().font_semibold().child("Groups")).child(
            Checkbox::new("filter-all-groups")
                .label("All groups")
                .checked(self.draft.group_ids.is_empty())
                .disabled(!self.writable)
                .on_click(cx.listener(|dialog, _, w, cx| {
                    if !dialog.writable {
                        return;
                    }
                    dialog.draft.group_ids = if dialog.draft.group_ids.is_empty() {
                        dialog.groups.iter().map(|g| g.0.clone()).collect()
                    } else {
                        Vec::new()
                    };
                    dialog.changed(w, cx);
                })),
        );
        for (id, name) in &self.groups {
            let value = id.clone();
            let selected = self.draft.group_ids.is_empty() || self.draft.group_ids.contains(id);
            let last = selected && (self.groups.len() == 1 || self.draft.group_ids.len() == 1);
            body = body.child(
                Checkbox::new(key(format!("filter-group-{id}")))
                    .label(name.clone())
                    .checked(selected)
                    .disabled(!self.writable || last)
                    .on_click(cx.listener(move |dialog, _, w, cx| {
                        dialog.toggle_group(value.clone(), w, cx)
                    })),
            );
        }
        for id in self
            .draft
            .group_ids
            .iter()
            .filter(|id| !self.groups.iter().any(|g| &g.0 == *id))
        {
            let value = id.clone();
            body = body.child(
                Checkbox::new(key(format!("filter-group-{id}")))
                    .label("Unavailable group")
                    .checked(true)
                    .disabled(!self.writable)
                    .on_click(cx.listener(move |dialog, _, w, cx| {
                        dialog.draft.group_ids.retain(|id| id != &value);
                        dialog.changed(w, cx);
                    })),
            );
        }
        body.child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("filter-discard")
                        .label("Discard draft")
                        .disabled(!self.writable)
                        .on_click(cx.listener(|dialog, _, w, cx| {
                            dialog.finish(true, cx);
                            w.close_dialog(cx);
                        })),
                )
                .child(
                    Button::new("filter-apply")
                        .label("Apply")
                        .primary()
                        .on_click(cx.listener(|dialog, _, w, cx| {
                            dialog.finish(false, cx);
                            w.close_dialog(cx);
                        })),
                ),
        )
    }
}

#[derive(Clone)]
struct SaveDraft {
    id: Uuid,
    name: String,
    filter: String,
    filters: ViewFilters,
}

pub(super) struct ViewsDialog {
    owner: WeakEntity<Table>,
    binding: String,
    board: Uuid,
    writable: bool,
    name: Entity<InputState>,
    views: Vec<SavedView>,
    pending: bool,
    uncertain: Option<SaveDraft>,
    proposed: Uuid,
    notice: String,
    generation: Uuid,
    request: Option<Task<()>>,
}

impl ViewsDialog {
    fn access_error(&self, error: &EditError, cx: &mut Context<Self>) {
        if matches!(error, EditError::PermissionDenied | EditError::Missing) {
            let _ = self.owner.update(cx, |_, cx| cx.emit(AccessLost));
        }
    }

    fn accept_list(&mut self, views: Vec<SavedView>, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(draft) = &self.uncertain {
            if let Some(saved) = views.iter().find(|view| view.id == draft.id) {
                let mut wanted = draft.filters.clone();
                wanted.status_values.sort();
                wanted.status_values.dedup();
                wanted.group_ids.sort();
                wanted.group_ids.dedup();
                if !saved.shared
                    && saved.name == draft.name.trim()
                    && saved.filter == draft.filter.trim()
                    && saved.filters == wanted
                {
                    self.notice = "View saved.".into();
                    self.uncertain = None;
                    self.proposed = Uuid::new_v4();
                    self.name.update(cx, |name, cx| name.set_value("", w, cx));
                } else {
                    self.notice = "This view request has different saved settings. Close and inspect the saved view.".into();
                }
            } else {
                self.uncertain = None;
                self.notice = "The view was not saved. Your name and filters are kept; choose Save view to try again.".into();
            }
        }
        self.views = views;
        self.pending = false;
        cx.notify();
    }

    fn reload(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        self.pending = true;
        self.notice = "Loading views…".into();
        self.generation = Uuid::new_v4();
        let generation = self.generation;
        let binding = self.binding.clone();
        let board = self.board;
        self.request = Some(cx.spawn_in(w, async move |dialog, cx| {
            let result = session::list_views(&binding, board, cx.background_executor()).await;
            let _ = dialog.update_in(cx, |dialog, w, cx| {
                if dialog.generation != generation {
                    return;
                }
                match result {
                    Ok(views) => {
                        dialog.notice.clear();
                        dialog.accept_list(views, w, cx);
                    }
                    Err(error) => {
                        dialog.pending = false;
                        dialog.notice =
                            "Views could not be read. Reload views to check the latest result."
                                .into();
                        dialog.access_error(&error, cx);
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn save(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.pending || self.uncertain.is_some() || !self.writable {
            return;
        }
        let name = self.name.read(cx).value().to_string();
        if name.trim().is_empty() {
            self.notice = "Enter a view name.".into();
            cx.notify();
            return;
        }
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let table = owner.read(cx);
        if !table.read.is_editable() {
            return;
        }
        let draft = SaveDraft {
            id: self.proposed,
            name,
            filter: table.search.read(cx).value().to_string(),
            filters: table.view_filters.clone(),
        };
        self.uncertain = Some(draft.clone());
        self.pending = true;
        self.notice = "Saving view…".into();
        self.generation = Uuid::new_v4();
        let generation = self.generation;
        let binding = self.binding.clone();
        let board = self.board;
        self.request = Some(cx.spawn_in(w, async move |dialog, cx| {
            let result = session::save_view(&binding, board, draft.id, &draft.name, &draft.filter, &draft.filters, cx.background_executor()).await;
            // A dropped response is ambiguous. Read the list once; never replay the POST.
            let result = match result {
                Ok(saved) => Ok(vec![saved]),
                Err(error) => {
                    let _ = dialog.update_in(cx, |dialog, _, cx| {
                        if dialog.generation == generation { dialog.access_error(&error, cx); }
                    });
                    session::list_views(&binding, board, cx.background_executor()).await
                }
            };
            let _ = dialog.update_in(cx, |dialog, w, cx| {
                if dialog.generation != generation { return; }
                match result {
                    Ok(views) => {
                        // Merge the confirmed result into the list already shown in this dialog.
                        let mut combined = dialog.views.clone();
                        for view in views { combined.retain(|old| old.id != view.id); combined.push(view); }
                        dialog.accept_list(combined, w, cx);
                    }
                    Err(error) => {
                        dialog.pending = false;
                        dialog.notice = "The save result could not be checked. Your name and filters are kept. Reload views before saving again.".into();
                        dialog.access_error(&error, cx);
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn close(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        let _ = self.owner.update(cx, |table, cx| {
            table.views_open = false;
            cx.notify();
        });
        w.close_dialog(cx);
    }

    fn apply(&mut self, view: SavedView, w: &mut Window, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        let _ = self.owner.update(cx, |table, cx| {
            table.search_task = None;
            table.search.update(cx, |search, cx| {
                search.set_value(view.filter.clone(), w, cx)
            });
            table.search_task = None;
            table.result_search = view.filter.trim().to_lowercase();
            table.search_invalid = false;
            table.view_filters = view.filters;
            table.drag.borrow_mut().cancel();
            for drag in table.row_drags.values() {
                drag.borrow_mut().cancel();
            }
            cx.notify();
        });
        self.close(w, cx);
    }
}

impl Render for ViewsDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .capture_key_down(
                cx.listener(|dialog, event: &gpui_kit::KeyDownEvent, w, cx| {
                    if event.keystroke.key == "escape" {
                        dialog.close(w, cx);
                        cx.stop_propagation();
                    }
                }),
            );
        if self.views.is_empty() && !self.pending {
            body = body.child(status("No saved views yet."));
        }
        for view in &self.views {
            let saved = view.clone();
            body = body.child(
                Button::new(key(format!("saved-view-{}", view.id)))
                    .label(if view.shared {
                        format!("{} · Shared", view.name)
                    } else {
                        view.name.clone()
                    })
                    .w_full()
                    .disabled(self.pending)
                    .on_click(
                        cx.listener(move |dialog, _, w, cx| dialog.apply(saved.clone(), w, cx)),
                    ),
            );
        }
        if self.writable {
            body = body
                .child(div().font_semibold().child("Save current view"))
                .child(
                    Input::new(&self.name)
                        .id("view-name")
                        .aria_label("View name")
                        .disabled(self.pending || self.uncertain.is_some()),
                )
                .child(
                    Button::new("save-view")
                        .label("Save view")
                        .primary()
                        .disabled(self.pending || self.uncertain.is_some())
                        .on_click(cx.listener(|dialog, _, w, cx| dialog.save(w, cx))),
                );
        }
        body.child(status(self.notice.clone())).child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("reload-views")
                        .label("Reload views")
                        .disabled(self.pending)
                        .on_click(cx.listener(|dialog, _, w, cx| dialog.reload(w, cx))),
                )
                .child(
                    Button::new("close-views")
                        .label("Close")
                        .disabled(self.pending)
                        .on_click(cx.listener(|dialog, _, w, cx| dialog.close(w, cx))),
                ),
        )
    }
}

impl Table {
    pub(super) fn matches_view_filters(&self, id: Uuid) -> bool {
        let Some(group) = self
            .read
            .groups()
            .iter()
            .find(|group| group.items().iter().any(|row| row.id == id))
        else {
            return false;
        };
        let filtered_status =
            self.view_filters.include_empty_status || !self.view_filters.status_values.is_empty();
        let cell = self
            .status
            .snapshot()
            .and_then(|table| table.cells.iter().find(|cell| cell.item_id == id));
        if filtered_status && cell.is_none() {
            return false;
        }
        self.view_filters
            .matches(cell.and_then(|cell| cell.value.as_deref()), group.id())
    }

    pub(super) fn open_filters(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.filter_focus.focus(w, cx);
        let rows = self
            .read
            .groups()
            .iter()
            .flat_map(|group| {
                group
                    .items()
                    .iter()
                    .map(|row| (row.id, group.id().to_owned(), row.title.clone()))
            })
            .filter(|(_, _, title)| {
                self.result_search.is_empty() || title.to_lowercase().contains(&self.result_search)
            })
            .collect();
        let owner = cx.weak_entity();
        let dialog = cx.new(|_| FilterDialog {
            owner,
            baseline: self.view_filters.clone(),
            draft: self.view_filters.clone(),
            writable: self.read.is_editable(),
            groups: self
                .read
                .groups()
                .iter()
                .map(|g| (g.id().to_owned(), g.name().to_owned()))
                .collect(),
            rows,
            status: self.status.snapshot().cloned(),
            closed: false,
            preview: None,
        });
        self.filter_dialog = Some(dialog.clone());
        w.open_dialog(cx, move |shell, _, _| {
            let closed = dialog.downgrade();
            shell
                .title("Quick filters")
                .child(dialog.clone())
                .on_close(move |_, _, cx| {
                    let _ = closed.update(cx, |dialog, cx| dialog.finish(false, cx));
                })
        });
        cx.notify();
    }

    pub(super) fn open_views(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.views_focus.focus(w, cx);
        let dialog = if let Some(dialog) = &self.saved_views {
            dialog.clone()
        } else {
            let owner = cx.weak_entity();
            let dialog = cx.new(|cx| ViewsDialog {
                owner,
                binding: self.binding.clone(),
                board: self.board,
                writable: self.read.is_editable(),
                name: cx.new(|cx| InputState::new(w, cx).placeholder("View name")),
                views: Vec::new(),
                pending: false,
                uncertain: None,
                proposed: Uuid::new_v4(),
                notice: String::new(),
                generation: Uuid::new_v4(),
                request: None,
            });
            self.saved_views = Some(dialog.clone());
            dialog
        };
        self.views_open = true;
        dialog.update(cx, |dialog, _| dialog.writable = self.read.is_editable());
        #[cfg(target_family = "wasm")]
        dialog.update(cx, |dialog, cx| dialog.reload(w, cx));
        let owner = cx.weak_entity();
        w.open_dialog(cx, move |shell, _, cx| {
            let owner = owner.clone();
            shell
                .title("Saved views")
                .overlay_closable(false)
                .keyboard(false)
                .close_button(!dialog.read(cx).pending)
                .child(dialog.clone())
                .on_close(move |_, _, cx| {
                    let _ = owner.update(cx, |table, cx| {
                        table.views_open = false;
                        cx.notify();
                    });
                })
        });
        cx.notify();
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use crate::groups::tests::visit_titles;
    use boardcraft_mobile_edit_client::{StatusCell, StatusChoice};
    use gpui_kit::{
        TestAppContext,
        test::{TestEventExt, TestWindowExt},
    };

    fn statuses() -> StatusTable {
        StatusTable {
            column_id: Uuid::from_u128(500),
            title: "Status".into(),
            choices: vec![StatusChoice {
                key: "done".into(),
                label: "Done".into(),
                color: "#00c875".into(),
            }],
            cells: vec![
                StatusCell {
                    item_id: Uuid::from_u128(59),
                    revision: 1,
                    value: Some("done".into()),
                },
                StatusCell {
                    item_id: Uuid::from_u128(61),
                    revision: 1,
                    value: None,
                },
            ],
        }
    }

    fn advance(cx: &mut TestAppContext, millis: u64) {
        cx.run_until_parked();
        cx.background_executor
            .advance_clock(Duration::from_millis(millis));
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn quick_filters_preview_after_200ms_and_discard_restores_opening_filters(
        cx: &mut TestAppContext,
    ) {
        let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
        cx.update(|cx| cx.set_reduce_motion(true));
        let sketch = Uuid::from_u128(61);
        cx.update_window(handle, |_, w, cx| {
            table.update(cx, |table, cx| table.status.accept(statuses(), cx));
            w.render_frame(cx);
            w.click(format!("check-{sketch}"), cx);
            w.click("add-input-g-b", cx);
            w.input("Unsubmitted item", cx);
            w.click("board-filter", cx);
            assert_eq!(w.find("filter-status-done").label(), Some("Done (1)"));
            w.click("filter-status-done", cx);
            assert_eq!(table.read(cx).view_filters, ViewFilters::default());
        })
        .unwrap();
        advance(cx, 199);
        cx.update(|cx| assert_eq!(table.read(cx).view_filters, ViewFilters::default()));
        advance(cx, 1);
        cx.update_window(handle, |_, w, cx| {
            assert_eq!(table.read(cx).view_filters.status_values, ["done"]);
            w.click("filter-apply", cx);
            assert_eq!(w.find("board-filter").focused(), Some(true));
            assert!(w.try_find(format!("item-{sketch}")).is_none());
            assert_eq!(table.read(cx).selection, [sketch]);
            assert_eq!(
                table.read(cx).entries["g-b"].input.read(cx).value(),
                "Unsubmitted item"
            );
        })
        .unwrap();
        advance(cx, 200);
        cx.update_window(handle, |_, w, cx| {
            w.click("board-filter", cx);
            w.click("filter-status-blank", cx);
            assert!(
                table
                    .read(cx)
                    .filter_dialog
                    .as_ref()
                    .unwrap()
                    .read(cx)
                    .draft
                    .include_empty_status
            );
        })
        .unwrap();
        advance(cx, 200);
        cx.update_window(handle, |_, w, cx| {
            assert!(table.read(cx).view_filters.include_empty_status);
            assert!(table.read(cx).view_filters.status_values.is_empty());
            w.click("filter-discard", cx);
            assert_eq!(table.read(cx).view_filters.status_values, ["done"]);
            assert!(!table.read(cx).view_filters.include_empty_status);
            assert_eq!(table.read(cx).selection, [sketch]);
        })
        .unwrap();
        advance(cx, 300);
        cx.update(|cx| assert_eq!(table.read(cx).view_filters.status_values, ["done"]));
    }

    #[gpui_kit::test]
    fn escape_flushes_preview_and_group_filter_keeps_last_group(cx: &mut TestAppContext) {
        let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
        cx.update(|cx| cx.set_reduce_motion(true));
        cx.update_window(handle, |_, w, cx| {
            table.update(cx, |table, cx| table.status.accept(statuses(), cx));
            w.render_frame(cx);
            w.click("board-filter", cx);
            w.click("filter-group-g-a", cx);
            assert_eq!(w.find("filter-group-g-b").disabled(), Some(true));
            w.press("escape", cx);
            assert!(table.read(cx).filter_dialog.is_none());
            assert_eq!(table.read(cx).view_filters.group_ids, ["g-b"]);
            assert!(w.try_find("heading-g-a").is_none());
            assert!(w.try_find("heading-g-b").is_some());
            assert_eq!(w.find("board-filter").focused(), Some(true));
        })
        .unwrap();
        advance(cx, 300);
        cx.update_window(handle, |_, w, cx| {
            w.click("board-filter", cx);
            w.click("filter-clear", cx);
            let outside = gpui_kit::point(gpui_kit::px(100.), gpui_kit::px(60.));
            w.pointer_down(outside, gpui_kit::MouseButton::Left, cx);
            w.pointer_up(outside, gpui_kit::MouseButton::Left, cx);
            assert!(table.read(cx).filter_dialog.is_none());
            assert_eq!(table.read(cx).view_filters, ViewFilters::default());
            assert!(w.try_find("heading-g-a").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn readonly_filters_and_saved_shared_view_preserve_drafts_and_unknown_blank(
        cx: &mut TestAppContext,
    ) {
        let (handle, table) = visit_titles(cx, false, ["Ink", "Sketch"]);
        let id = Uuid::from_u128(700);
        cx.update_window(handle, |_, w, cx| {
            w.render_frame(cx);
            w.click("board-filter", cx);
            assert_eq!(w.find("filter-all-groups").disabled(), Some(true));
            assert_eq!(w.find("filter-clear").disabled(), Some(true));
            w.press("escape", cx);
            w.click("board-views", cx);
            assert!(w.try_find("save-view").is_none());
            let dialog = table.read(cx).saved_views.clone().unwrap();
            dialog.update(cx, |dialog, cx| {
                dialog.views.push(SavedView {
                    id,
                    name: "Blank sketches".into(),
                    filter: "Sketch".into(),
                    shared: true,
                    owner: Uuid::from_u128(900),
                    filters: ViewFilters {
                        include_empty_status: true,
                        ..Default::default()
                    },
                });
                cx.notify();
            });
            w.render_frame(cx);
            assert_eq!(
                w.find(format!("saved-view-{id}")).label(),
                Some("Blank sketches · Shared")
            );
            w.click(format!("saved-view-{id}"), cx);
            assert_eq!(table.read(cx).result_search, "sketch");
            assert!(
                table.read(cx).matching_order(cx).is_empty(),
                "unread Status cells are not known Blank values"
            );
            table.update(cx, |table, cx| table.status.accept(statuses(), cx));
            assert_eq!(table.read(cx).matching_order(cx), [Uuid::from_u128(61)]);
            w.click("board-views", cx);
            assert!(w.try_find(format!("saved-view-{id}")).is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn saved_view_validation_and_uncertain_read_keep_name_until_request_is_confirmed(
        cx: &mut TestAppContext,
    ) {
        let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
        cx.update_window(handle, |_, w, cx| {
            w.render_frame(cx);
            w.click("board-views", cx);
            w.click("save-view", cx);
            let dialog = table.read(cx).saved_views.clone().unwrap();
            assert_eq!(dialog.read(cx).notice, "Enter a view name.");
            assert!(!dialog.read(cx).pending);
            w.click("view-name", cx);
            w.input("My current view", cx);
            w.click("close-views", cx);
            w.click("board-views", cx);
            assert_eq!(w.find("view-name").value(), Some("My current view"));
            let original = dialog.read(cx).proposed;
            dialog.update(cx, |dialog, cx| {
                dialog.uncertain = Some(SaveDraft {
                    id: original,
                    name: "My current view".into(),
                    filter: "Sketch".into(),
                    filters: ViewFilters::default(),
                });
                dialog.accept_list(Vec::new(), w, cx);
                assert!(dialog.uncertain.is_none());
                assert_eq!(dialog.proposed, original);
                assert_eq!(dialog.name.read(cx).value(), "My current view");
                dialog.uncertain = Some(SaveDraft {
                    id: original,
                    name: "My current view".into(),
                    filter: "Sketch".into(),
                    filters: ViewFilters::default(),
                });
                dialog.accept_list(
                    vec![SavedView {
                        id: original,
                        name: "My current view".into(),
                        filter: "Sketch".into(),
                        filters: ViewFilters::default(),
                        shared: false,
                        owner: Uuid::from_u128(900),
                    }],
                    w,
                    cx,
                );
                assert!(dialog.uncertain.is_none());
                assert_ne!(dialog.proposed, original);
                assert_eq!(dialog.name.read(cx).value(), "");
                assert_eq!(dialog.notice, "View saved.");
                assert!(
                    dialog.request.is_none(),
                    "reconciliation must not schedule a replay"
                );
            });
        })
        .unwrap();
    }
}
