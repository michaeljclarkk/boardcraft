use super::*;
use boardcraft_mobile_edit_client::{StatusCell, StatusChoice};
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

/// One opened native view-settings dialog owns its draft and debounce. The board
/// receives preview values; closing keeps them, while Discard restores the baseline.
struct FilterDialog {
    owner: WeakEntity<BoardView>,
    board: Uuid,
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
            || status.pending.is_some()
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
            draft: baseline.clone(),
            baseline,
            groups: groups.clone(),
            rows: rows.clone(),
            choices: status.choices.to_vec(),
            cells: status.cells.clone(),
            status_title: status.title.clone(),
            query: filter.clone(),
            section: 0,
            writable: status.editable,
            dirty: false,
            closed: false,
            preview: None,
        });
        self.filters_open = true;
        self.search_focused = false;
        self.filter_focus.focus(window, cx);
        window.open_dialog(cx, move |surface, window, cx| {
            let close = dialog.clone();
            let dismiss = dialog.clone();
            let actions = dialog.update(cx, |dialog, cx| dialog.footer(cx));
            surface
                .w(window.viewport_size().width)
                .centered()
                .close_button(false)
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
                                .on_click(move |_, window, cx| {
                                    close.update(cx, |dialog, cx| dialog.finish(false, cx));
                                    window.close_dialog(cx);
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
    fn publish(&self, filters: &Filters, cx: &mut Context<Self>) {
        let _ = self.owner.update(cx, |view, cx| {
            if !view.filters_open || !matches!(view.navigation, Some(Page::Selected { board, .. }) if board == self.board) {
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
            view.filters_open = false;
            cx.notify();
        });
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
                    .on_click(cx.listener(|dialog, _, window, cx| {
                        dialog.finish(true, cx);
                        window.close_dialog(cx);
                    })),
            )
            .child(
                Button::new("mobile-filter-apply")
                    .large()
                    .primary()
                    .flex_1()
                    .label("Apply")
                    .on_click(cx.listener(|dialog, _, window, cx| {
                        dialog.finish(false, cx);
                        window.close_dialog(cx);
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
                    .disabled(!self.writable)
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
                .disabled(!self.writable)
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
                    .disabled(!self.writable)
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
                        !self.writable
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
                    .child(Tab::new().label("Group by"))
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
                        "Group by"
                    })))
                    .child(
                        Button::new("mobile-filter-clear")
                            .large()
                            .label("Clear all")
                            .disabled(!self.writable)
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
    }
}
