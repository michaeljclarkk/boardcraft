//! Selected cells save independently. Reconciliation reads accepted values and never replays a choice.
use super::*;
use gpui_kit::component::{
    IndexPath,
    select::{Select, SelectEvent, SelectState},
};

pub(super) struct BulkStatus {
    column: Entity<SelectState<Vec<SharedString>>>,
    trigger: FocusHandle,
    initial: FocusHandle,
    request: Option<Uuid>,
    palette: Option<BulkTarget>,
    pub(super) pending: Vec<Uuid>,
    pub(super) failures: HashMap<Uuid, String>,
    needs_review: bool,
    choice: String,
    _events: Subscription,
}

impl BulkStatus {
    pub(super) fn new(w: &mut Window, cx: &mut Context<Connected>) -> Self {
        let column = cx.new(|cx| {
            SelectState::new(
                vec!["Change label…".into(), "Status".into()],
                Some(IndexPath::new(0)),
                w,
                cx,
            )
        });
        let events = cx.subscribe(&column, |_, _, _: &SelectEvent<Vec<SharedString>>, cx| {
            cx.notify()
        });
        Self {
            column,
            trigger: cx.focus_handle().tab_stop(true),
            initial: cx.focus_handle().tab_stop(true),
            request: None,
            palette: None,
            pending: Vec::new(),
            failures: HashMap::new(),
            needs_review: false,
            choice: String::new(),
            _events: events,
        }
    }
}

#[derive(Clone)]
struct BulkTarget {
    visit: Uuid,
    column: Uuid,
    cells: Vec<(Uuid, i64)>,
}

impl MainTable {
    pub(in crate::connected::main_table) fn selection_status_controls(
        &self,
        cx: &mut Context<Connected>,
    ) -> impl IntoElement {
        let disabled =
            !self.editable || self.view_actions_blocked(cx) || !self.status.filters_ready();
        let mut controls = div().flex().h_full().items_start().flex_none().child(
            Select::new(&self.status.bulk.column)
                .id("selection-status-column")
                .accessibility_label("Bulk edit status column")
                .small()
                .h(rems(4.25))
                .w(rems(9.5))
                .disabled(disabled),
        );
        if self
            .status
            .bulk
            .column
            .read(cx)
            .selected_value()
            .is_some_and(|value| value.as_ref() == "Status")
        {
            controls = controls.child(self.bulk_status_picker(disabled, cx));
        }
        controls
    }

    pub(in crate::connected::main_table) fn bulk_status_failures(
        &self,
        cx: &mut Context<Connected>,
    ) -> Option<AnyElement> {
        if !self.status.bulk.failures.is_empty() {
            let mut failures = div()
                .id("selection-status-failures")
                .flex()
                .flex_col()
                .gap_1();
            for heading in &self.headings {
                for row in &heading.rows {
                    let row = row.read(cx);
                    let item = row.edit.identity().1.as_uuid();
                    if let Some(reason) = self.status.bulk.failures.get(&item) {
                        failures = failures.child(
                            Label::new(format!("{}: {reason}", row.edit.accepted()))
                                .w_full()
                                .whitespace_normal()
                                .id(SharedString::from(format!(
                                    "selection-status-failure-{item}"
                                ))),
                        );
                    }
                }
            }
            return Some(
                failures
                    .child(
                        Button::new("selection-status-review")
                            .small()
                            .label("Review statuses")
                            .disabled(self.pending)
                            .on_click(cx.listener(|view, _, w, cx| view.review_bulk_status(w, cx))),
                    )
                    .into_any_element(),
            );
        }
        None
    }

    fn bulk_status_picker(&self, disabled: bool, cx: &mut Context<Connected>) -> AnyElement {
        let button = Button::new("selection-status-picker")
            .label("Choose label")
            .track_focus(&self.status.bulk.trigger)
            .w(rems(7.5))
            .h(rems(2.125))
            .min_h(rems(2.125))
            .rounded_sm()
            .custom(tile_style(rgb(0x757575).into(), cx))
            .disabled(disabled);
        let ColumnRead::Ready {
            column,
            choices,
            cells,
            ..
        } = &self.status.read
        else {
            return button.into_any_element();
        };
        let targets: Option<Vec<_>> = self
            .selection
            .ids
            .iter()
            .map(|id| cells.get(id).map(|cell| (*id, cell.saved.revision)))
            .collect();
        let Some(targets) = targets else {
            return button.disabled(true).into_any_element();
        };
        if disabled {
            return button.into_any_element();
        }
        let target = BulkTarget {
            visit: self.status.visit,
            column: *column,
            cells: targets,
        };
        let choices = Rc::clone(choices);
        let owner = cx.weak_entity();
        let opening = owner.clone();
        let opening_target = target.clone();
        let initial = self.status.bulk.initial.clone();
        let focus = self.status.bulk.trigger.clone();
        Popover::new("selection-status-popup")
            .track_focus(&initial)
            .on_open_change(move |open, _, cx| {
                let _ = opening.update(cx, |view, cx| {
                    if let Screen::Navigation(nav) = &mut view.screen
                        && let Page::Items(_, table) = &mut nav.page
                        && table.status.visit == opening_target.visit
                    {
                        table.status.bulk.palette = open.then(|| opening_target.clone());
                        cx.notify();
                    }
                });
            })
            .trigger(button.on_mouse_down(MouseButton::Left, move |_, w, cx| focus.focus(w, cx)))
            .content(move |_, w, cx| {
                let target = owner
                    .upgrade()
                    .and_then(|owner| {
                        let Screen::Navigation(nav) = &owner.read(cx).screen else {
                            return None;
                        };
                        let Page::Items(_, table) = &nav.page else {
                            return None;
                        };
                        table.status.bulk.palette.clone()
                    })
                    .unwrap_or_else(|| target.clone());
                let popup = cx.weak_entity();
                let commit = |value: Option<String>| {
                    let owner = owner.clone();
                    let popup = popup.clone();
                    let target = target.clone();
                    move |_: &ClickEvent, w: &mut Window, cx: &mut App| {
                        let _ =
                            popup.update(cx, |popup: &mut PopoverState, cx| popup.dismiss(w, cx));
                        let _ = owner.update(cx, |view, cx| {
                            view.set_bulk_status(target.clone(), value.clone(), w, cx)
                        });
                    }
                };
                let mut options = div()
                    .id("selection-status-options")
                    .role(Role::ListBox)
                    .aria_label("Choose label")
                    .flex()
                    .flex_col()
                    .gap_2();
                let mut row = div().flex().gap_2();
                let mut handles = Vec::new();
                for (index, choice) in choices
                    .iter()
                    .map(Some)
                    .chain(std::iter::once(None))
                    .enumerate()
                {
                    let id: SharedString = choice.map_or_else(
                        || "selection-status-empty".into(),
                        |c| format!("selection-status-choice-{}", c.key).into(),
                    );
                    let handle = if index == 0 {
                        initial.clone()
                    } else {
                        w.use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle().tab_stop(true))
                            .read(cx)
                            .clone()
                    };
                    let bg = choice.map_or_else(|| rgb(0xc4c4c4).into(), |c| color(c, cx));
                    row = row.child(
                        Button::new(id)
                            .role(Role::ListBoxOption)
                            .track_focus(&handle)
                            .flex_1()
                            .min_w_0()
                            .h_8()
                            .rounded_sm()
                            .custom(tile_style(bg, cx))
                            .label(choice.map_or("Empty", |c| c.label.as_str()).to_owned())
                            .on_click(commit(choice.map(|c| c.key.clone()))),
                    );
                    handles.push(handle);
                    if index % 2 == 1 || index == choices.len() {
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
                        Button::new("selection-status-clear")
                            .w_full()
                            .h_8()
                            .label("Clear value")
                            .on_click(commit(None)),
                    )
                    .on_key_down(move |event: &KeyDownEvent, w, cx| {
                        let Some(index) = handles.iter().position(|handle| handle.is_focused(w))
                        else {
                            return;
                        };
                        let count = handles.len();
                        let next = match event.keystroke.key.as_str() {
                            "down" => (index + 1) % count,
                            "up" => (index + count - 1) % count,
                            "home" => 0,
                            "end" => count - 1,
                            _ => return,
                        };
                        handles[next].focus(w, cx);
                        cx.stop_propagation();
                    })
            })
            .into_any_element()
    }
}

impl Connected {
    fn set_bulk_status(
        &mut self,
        target: BulkTarget,
        value: Option<String>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, table) = &mut nav.page else {
            return;
        };
        if table.status.visit != target.visit
            || !table.editable
            || table.view_actions_blocked(cx)
            || !table.status.filters_ready()
            || !target
                .cells
                .iter()
                .map(|(id, _)| *id)
                .eq(table.selection.ids.iter().copied())
        {
            return;
        }
        let ColumnRead::Ready {
            column,
            cells,
            choices,
            ..
        } = &table.status.read
        else {
            return;
        };
        if *column != target.column
            || value
                .as_ref()
                .is_some_and(|value| !choices.iter().any(|choice| &choice.key == value))
        {
            return;
        }
        table.status.bulk.trigger.focus(w, cx);
        let changed: Vec<_> = target
            .cells
            .iter()
            .filter(|(id, _)| cells.get(id).is_some_and(|cell| cell.saved.value != value))
            .copied()
            .collect();
        for (id, _) in &target.cells {
            table.status.bulk.failures.remove(id);
        }
        if changed.is_empty() {
            cx.notify();
            return;
        }
        let request = Uuid::new_v4();
        table.status.bulk.request = Some(request);
        table.status.bulk.pending = changed.iter().map(|(id, _)| *id).collect();
        table.status.bulk.needs_review = false;
        table.status.bulk.choice = choices
            .iter()
            .find(|c| Some(&c.key) == value.as_ref())
            .map_or("Clear", |c| c.label.as_str())
            .to_owned();
        table.set_busy(true, cx);
        let board = *board;
        let client = nav.client.clone();
        for (item, revision) in changed {
            let client = client.clone();
            let value = value.clone();
            let visit = target.visit;
            let column = target.column;
            let work = cx
                .background_executor()
                .spawn(async move { client.set_status(board, column, item, revision, value) });
            cx.spawn_in(w, async move |view, cx| {
                let result = work.await;
                let _ = view.update_in(cx, |view, w, cx| {
                    view.finish_bulk_cell(visit, request, item, result, w, cx)
                });
            })
            .detach();
        }
        cx.notify();
    }

    fn finish_bulk_cell(
        &mut self,
        visit: Uuid,
        request: Uuid,
        item: Uuid,
        result: Result<StatusCell, EditError>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(_, table) = &mut nav.page else {
            return;
        };
        if table.status.visit != visit
            || table.status.bulk.request != Some(request)
            || !table.status.bulk.pending.contains(&item)
        {
            return;
        }
        table.status.bulk.pending.retain(|id| *id != item);
        if matches!(result, Err(EditError::PermissionDenied)) {
            table.editable = false;
        }
        match result {
            Ok(saved) if saved.item_id == item => {
                if let ColumnRead::Ready { cells, .. } = &mut table.status.read
                    && let Some(cell) = cells.get_mut(&item)
                {
                    cell.saved = saved;
                }
            }
            Err(EditError::SessionRejected) => {
                self.finish_status_read(visit, Err(EditError::SessionRejected), w, cx);
                return;
            }
            result => {
                let reason = match result {
                    Err(EditError::Conflict) => "Status changed elsewhere",
                    Err(EditError::PermissionDenied) => "Permission denied",
                    _ => "Status not confirmed",
                };
                table.status.bulk.failures.insert(
                    item,
                    format!(
                        "{reason}. Review the current value before choosing {} again.",
                        table.status.bulk.choice
                    ),
                );
                table.status.bulk.needs_review = true;
                table.status.requires_reload = true;
            }
        }
        if table.status.bulk.pending.is_empty() {
            if table.status.bulk.needs_review {
                self.review_bulk_status(w, cx);
            } else {
                table.status.bulk.request = None;
                table.set_busy(false, cx);
            }
        }
        cx.notify();
    }

    fn review_bulk_status(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, table) = &mut nav.page else {
            return;
        };
        if !table.status.bulk.pending.is_empty() {
            return;
        }
        let request = Uuid::new_v4();
        table.status.bulk.request = Some(request);
        table.set_busy(true, cx);
        let visit = table.status.visit;
        let board = *board;
        let client = nav.client.clone();
        let work = cx
            .background_executor()
            .spawn(async move { client.read_status(board) });
        cx.spawn_in(w, async move |view, cx| {
            let result = work.await;
            let _ = view.update_in(cx, |view, w, cx| {
                view.finish_bulk_review(visit, request, result, w, cx)
            });
        })
        .detach();
        cx.notify();
    }

    fn finish_bulk_review(
        &mut self,
        visit: Uuid,
        request: Uuid,
        result: Result<StatusTable, EditError>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(_, table) = &mut nav.page else {
            return;
        };
        if table.status.visit != visit || table.status.bulk.request != Some(request) {
            return;
        }
        table.status.bulk.request = None;
        table.set_busy(false, cx);
        match result {
            Ok(read) => {
                table.status.accept(read, cx);
                table.status.bulk.needs_review = false;
            }
            Err(EditError::SessionRejected) => {
                self.finish_status_read(visit, Err(EditError::SessionRejected), w, cx);
                return;
            }
            Err(error) => {
                if error == EditError::PermissionDenied {
                    table.editable = false;
                    table.set_busy(false, cx);
                }
                table.status.requires_reload = true;
                self.message = if error == EditError::PermissionDenied {
                    "Could not review statuses: permission denied."
                } else {
                    "Could not review statuses. Review statuses again before changing labels."
                };
            }
        }
        cx.notify();
    }
}

#[cfg(all(test, feature = "native-smoke"))]
mod tests;
