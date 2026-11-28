//! The board visit owns accepted Status cells and their independent revisions.
//! An uncertain write is followed by a read; choosing again is always deliberate.
use super::*;
use boardcraft_mobile_edit_client::{StatusCell, StatusChoice, StatusEdit, StatusTable};
use gpui_kit::{
    AnyElement, ClickEvent, Hsla, KeyDownEvent, MouseButton, StyleRefinement,
    base::Selectable,
    component::{
        Colorize,
        button::{ButtonCustomVariant, ButtonVariants},
        popover::{Popover, PopoverState},
    },
    prelude::FluentBuilder,
    rems,
};
use std::cell::Cell;

pub(super) struct StatusColumn {
    generation: Uuid,
    accepted: Option<StatusTable>,
    cells: HashMap<Uuid, CellFocus>,
    loading: bool,
    failed: bool,
    notice: Option<String>,
    pending: Option<Uuid>,
    request: Option<Task<()>>,
}

struct CellFocus {
    trigger: FocusHandle,
    selected: FocusHandle,
    baseline: Rc<Cell<Option<Target>>>,
}

#[derive(Clone, Copy)]
struct Target {
    generation: Uuid,
    column: Uuid,
    item: Uuid,
    revision: i64,
}

impl StatusColumn {
    pub(super) fn new() -> Self {
        Self {
            generation: Uuid::new_v4(),
            accepted: None,
            cells: HashMap::new(),
            loading: true,
            failed: false,
            notice: None,
            pending: None,
            request: None,
        }
    }

    pub(super) fn accept(&mut self, read: StatusTable, cx: &mut App) {
        self.cells
            .retain(|id, _| read.cells.iter().any(|cell| cell.item_id == *id));
        for cell in &read.cells {
            self.cells.entry(cell.item_id).or_insert_with(|| CellFocus {
                trigger: cx.focus_handle().tab_stop(true),
                selected: cx.focus_handle().tab_stop(true),
                baseline: Rc::new(Cell::new(None)),
            });
        }
        self.accepted = Some(read);
        self.loading = false;
        self.failed = false;
    }

    pub(super) fn is_saving(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn snapshot(&self) -> Option<&StatusTable> {
        self.accepted.as_ref()
    }
}

fn lane() -> gpui_kit::Div {
    div().w_40().h_full().flex().items_center().justify_center()
}

fn color(choice: Option<&StatusChoice>, cx: &App) -> Hsla {
    // Label colors and the source empty grey are cell data, not chrome tokens.
    choice.map_or_else(
        || gpui_kit::rgb(0xc4c4c4).into(),
        |choice| Hsla::parse_hex(&choice.color).unwrap_or(cx.theme().muted_foreground),
    )
}

fn tile_style(bg: Hsla, cx: &App) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(bg)
        .foreground(gpui_kit::white())
        .hover(bg.darken(0.08))
        .active(bg.darken(0.15))
}

impl Table {
    pub(super) fn status_header(&self) -> impl IntoElement {
        lane().child(
            self.status
                .accepted
                .as_ref()
                .map_or("Status", |table| &table.title)
                .to_owned(),
        )
    }

    pub(super) fn status_cell(&self, item: Uuid, name: &str, cx: &mut Context<Self>) -> AnyElement {
        let Some(table) = &self.status.accepted else {
            return lane()
                .text_sm()
                .child(if self.status.loading {
                    "Loading…"
                } else {
                    "Unavailable"
                })
                .into_any_element();
        };
        let Some(cell) = table.cells.iter().find(|cell| cell.item_id == item) else {
            return lane().text_sm().child("Reload statuses").into_any_element();
        };
        let Some(focus) = self.status.cells.get(&item) else {
            return lane().into_any_element();
        };
        let selected = table
            .choices
            .iter()
            .find(|choice| Some(&choice.key) == cell.value.as_ref());
        let saving = self.status.pending == Some(item);
        let bg = if saving {
            gpui_kit::rgb(0x757575).into()
        } else {
            color(selected, cx)
        };
        let label = if saving {
            "Saving…"
        } else {
            selected.map_or("Empty", |choice| choice.label.as_str())
        };
        let trigger = Button::new(key(format!("status-{item}")))
            .track_focus(&focus.trigger)
            .custom(tile_style(bg, cx))
            .rounded_none()
            .bg(bg)
            .text_color(gpui_kit::white())
            .w_40()
            .h_full()
            .min_h_8()
            .label(
                if saving {
                    "Saving…"
                } else {
                    selected.map_or(" ", |choice| choice.label.as_str())
                }
                .to_owned(),
            )
            .accessibility_label(format!("{} for {name}: {label}", table.title));
        if self.busy() || !self.read.is_editable() || self.status.loading || self.status.failed {
            return lane().child(trigger.disabled(true)).into_any_element();
        }
        let target = Target {
            generation: self.status.generation,
            column: table.column_id,
            item,
            revision: cell.revision,
        };
        let view = cx.weak_entity();
        let choices = table.choices.clone();
        let value = cell.value.clone();
        let initial = focus.selected.clone();
        let return_focus = focus.trigger.clone();
        let baseline = focus.baseline.clone();
        let open_baseline = baseline.clone();
        let picker = Popover::new(key(format!("status-popup-{item}")))
            .trigger_style(StyleRefinement::default().w_full().h_full())
            .track_focus(&initial)
            .on_open_change(move |open, _, _| {
                if *open {
                    open_baseline.set(Some(target));
                }
            })
            .trigger(
                trigger.on_mouse_down(MouseButton::Left, move |_, w, cx| return_focus.focus(w, cx)),
            )
            .content(move |_, w, cx| {
                let popup = cx.weak_entity();
                let commit = |value: Option<String>| {
                    let view = view.clone();
                    let popup = popup.clone();
                    let baseline = baseline.clone();
                    move |_: &ClickEvent, w: &mut Window, cx: &mut App| {
                        let captured = baseline.get().unwrap_or(target);
                        let _ =
                            popup.update(cx, |popup: &mut PopoverState, cx| popup.dismiss(w, cx));
                        let _ = view.update(cx, |table, cx| {
                            table.set_status(captured, value.clone(), w, cx)
                        });
                    }
                };
                let mut options = div()
                    .id("status-options")
                    .role(Role::ListBox)
                    .aria_label("Choose Status")
                    .flex()
                    .flex_col()
                    .gap_2();
                let mut row = div().flex().gap_2();
                let mut handles = Vec::new();
                for (ix, option) in choices
                    .iter()
                    .map(Some)
                    .chain(std::iter::once(None))
                    .enumerate()
                {
                    let choice = option.map(|choice| choice.key.clone());
                    let checked = value == choice;
                    let id = key(option.map_or_else(
                        || "status-empty".into(),
                        |choice| format!("status-choice-{}", choice.key),
                    ));
                    let handle = if checked {
                        initial.clone()
                    } else {
                        w.use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle().tab_stop(true))
                            .read(cx)
                            .clone()
                    };
                    let bg = color(option, cx);
                    row = row.child(
                        Button::new(id)
                            .role(Role::ListBoxOption)
                            .track_focus(&handle)
                            .flex_1()
                            .min_w_0()
                            .h_8()
                            .label(
                                option
                                    .map_or("Empty", |choice| choice.label.as_str())
                                    .to_owned(),
                            )
                            .custom(tile_style(bg, cx))
                            .bg(bg)
                            .text_color(gpui_kit::white())
                            .selected(checked)
                            .toggled(checked)
                            .when(checked, |button| button.icon(IconName::Check))
                            .on_click(commit(choice)),
                    );
                    handles.push(handle);
                    if ix % 2 == 1 || ix == choices.len() {
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
                        Button::new("status-clear")
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
                            "right" => (ix + 1) % count,
                            "left" => (ix + count - 1) % count,
                            "down" => (ix + 1) % count,
                            "up" => (ix + count - 1) % count,
                            "home" => 0,
                            "end" => count - 1,
                            _ => return,
                        };
                        handles[next].focus(w, cx);
                        cx.stop_propagation();
                    })
            });
        lane().child(picker).into_any_element()
    }

    pub(super) fn status_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.status.is_saving() {
            return None;
        }
        let notice = self.status.notice.as_ref()?;
        Some(
            div()
                .id("status-notice")
                .role(Role::Status)
                .test_support()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .text_sm()
                .child(notice.clone())
                .children(self.status.failed.then(|| {
                    Button::new("reload-status")
                        .small()
                        .label("Reload statuses")
                        .disabled(self.pending || self.status.loading)
                        .on_click(cx.listener(|table, _, w, cx| table.load_status(w, cx)))
                }))
                .into_any_element(),
        )
    }

    pub(super) fn load_status(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        let generation = Uuid::new_v4();
        self.status.generation = generation;
        self.status.loading = true;
        let (binding, board, executor) = (
            self.binding.clone(),
            self.board,
            cx.background_executor().clone(),
        );
        self.status.request = Some(cx.spawn_in(w, async move |table, cx| {
            let result = session::read_status(&binding, board, &executor).await;
            let _ = table.update(cx, |table, cx| {
                table.finish_status_read(generation, result, cx)
            });
        }));
        cx.notify();
    }

    fn finish_status_read(
        &mut self,
        generation: Uuid,
        result: Result<StatusTable, EditError>,
        cx: &mut Context<Self>,
    ) {
        if self.status.generation != generation {
            return;
        }
        self.status.loading = false;
        match result {
            Ok(read) => {
                self.status.accept(read, cx);
                self.status.notice = None;
            }
            Err(error) => {
                self.status.failed = true;
                self.status.notice =
                    Some("Could not load statuses. Reload statuses to try again.".into());
                if matches!(
                    error,
                    EditError::SessionRejected | EditError::PermissionDenied
                ) {
                    cx.emit(AccessLost);
                }
            }
        }
        cx.notify();
    }

    fn set_status(
        &mut self,
        target: Target,
        value: Option<String>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if target.generation != self.status.generation
            || self.busy()
            || !self.read.is_editable()
            || self.status.loading
            || self.status.failed
        {
            return;
        }
        let Some(table) = &self.status.accepted else {
            return;
        };
        let Some(cell) = table.cells.iter().find(|cell| cell.item_id == target.item) else {
            return;
        };
        if let Some(focus) = self.status.cells.get(&target.item) {
            focus.trigger.focus(w, cx);
        }
        if cell.value == value {
            return;
        }
        let label = table
            .choices
            .iter()
            .find(|choice| Some(&choice.key) == value.as_ref())
            .map_or("Empty", |choice| choice.label.as_str())
            .to_owned();
        self.pending = true;
        self.status.pending = Some(target.item);
        self.status.notice = None;
        let (binding, board, executor) = (
            self.binding.clone(),
            self.board,
            cx.background_executor().clone(),
        );
        self.status.request = Some(cx.spawn_in(w, async move |table, cx| {
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
            let reread = if result.is_err()
                && !matches!(
                    result,
                    Err(EditError::SessionRejected | EditError::PermissionDenied)
                ) {
                Some(session::read_status(&binding, board, &executor).await)
            } else {
                None
            };
            let _ = table.update(cx, |table, cx| {
                table.finish_status_write(target, label, result, reread, cx)
            });
        }));
        cx.notify();
    }

    fn finish_status_write(
        &mut self,
        target: Target,
        label: String,
        result: Result<StatusCell, EditError>,
        reread: Option<Result<StatusTable, EditError>>,
        cx: &mut Context<Self>,
    ) {
        if self.status.generation != target.generation {
            return;
        }
        self.pending = false;
        self.status.pending = None;
        match result {
            Ok(saved) => {
                if let Some(table) = &mut self.status.accepted
                    && let Some(cell) = table
                        .cells
                        .iter_mut()
                        .find(|cell| cell.item_id == target.item)
                    && saved.revision >= cell.revision
                {
                    *cell = saved;
                }
                self.status.notice = None;
            }
            Err(EditError::SessionRejected | EditError::PermissionDenied) => {
                self.status.failed = true;
                self.status.notice = Some("Status not saved. Checking current access.".into());
                cx.emit(AccessLost);
            }
            Err(error) => {
                let confirmed = match reread {
                    Some(Ok(read)) => {
                        self.status.accept(read, cx);
                        true
                    }
                    Some(Err(EditError::SessionRejected | EditError::PermissionDenied)) => {
                        self.status.failed = true;
                        cx.emit(AccessLost);
                        false
                    }
                    _ => {
                        self.status.failed = true;
                        false
                    }
                };
                let reason = if error == EditError::Conflict {
                    "Status changed elsewhere"
                } else {
                    "Status not confirmed"
                };
                self.status.notice = Some(if confirmed {
                    format!("{reason}. Review the current value before choosing {label} again.")
                } else {
                    format!(
                        "{reason}. Reload statuses, then review the current value before choosing {label} again."
                    )
                });
            }
        }
        cx.notify();
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use crate::groups::tests::visit_titles;
    use gpui_kit::{TestAppContext, test::TestWindowExt};

    fn accepted(value: Option<&str>, revision: i64) -> StatusTable {
        StatusTable {
            column_id: Uuid::from_u128(500),
            title: "Status".into(),
            choices: [
                ("working", "Working on it", "#579bfc"),
                ("done", "Done", "#00c875"),
                ("stuck", "Stuck", "#fdab3d"),
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
    fn picker_preserves_selected_focus_and_always_offers_clear(cx: &mut TestAppContext) {
        let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
        let trigger = format!("status-{}", Uuid::from_u128(59));
        cx.update_window(handle, |_, w, cx| {
            table.update(cx, |table, cx| {
                table.status.accept(accepted(Some("stuck"), 2), cx)
            });
            w.render_frame(cx);
            let row = w.find(format!("row-{}", Uuid::from_u128(59))).bounds();
            let tile = w.find(trigger.clone()).bounds();
            assert!(
                (row.size.height - tile.size.height).abs() <= gpui_kit::px(1.),
                "row {row:?}, tile {tile:?}"
            );
            assert_eq!(row.top(), tile.top());
            w.click(trigger.clone(), cx);
            w.render_frame(cx);
            assert_eq!(w.find("status-choice-stuck").focused(), Some(true));
            assert_eq!(w.find("status-choice-stuck").checked(), Some(true));
            let working = w.find("status-choice-working").bounds();
            let done = w.find("status-choice-done").bounds();
            let stuck = w.find("status-choice-stuck").bounds();
            assert_eq!(working.top(), done.top());
            assert_eq!(working.left(), stuck.left());
            assert!(stuck.top() > working.bottom());
            w.press("up", cx);
            assert_eq!(w.find("status-choice-done").focused(), Some(true));
            w.press("right", cx);
            assert_eq!(w.find("status-choice-stuck").focused(), Some(true));
            w.press("escape", cx);
            assert!(w.try_find("status-clear").is_none());
            assert_eq!(w.find(trigger.clone()).focused(), Some(true));
            w.press("enter", cx);
            w.press("enter", cx); // Confirming the accepted Stuck value closes without a write.
            assert!(w.try_find("status-clear").is_none());
            assert!(!table.read(cx).pending);
            assert!(table.read(cx).status.request.is_none());
            table.update(cx, |table, cx| {
                table.status.accept(accepted(None, 3), cx);
                cx.notify();
            });
            w.render_frame(cx);
            w.click(trigger.clone(), cx);
            assert_eq!(w.find("status-empty").focused(), Some(true));
            assert!(w.find("status-clear").visible());
            w.click("status-clear", cx);
            assert_eq!(w.find(trigger.clone()).focused(), Some(true));
            assert!(!table.read(cx).pending);
            assert!(table.read(cx).status.request.is_none());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn saving_status_keeps_table_geometry_and_shows_progress_only_in_target_cell(
        cx: &mut TestAppContext,
    ) {
        let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
        let item = Uuid::from_u128(59);
        let other = Uuid::from_u128(61);
        cx.update_window(handle, |_, w, cx| {
            table.update(cx, |table, cx| table.status.accept(accepted(None, 0), cx));
            w.render_frame(cx);
            let before = w.find(format!("row-{item}")).bounds();
            let other_before = w.find(format!("row-{other}")).bounds();
            table.update(cx, |table, cx| {
                table.pending = true;
                table.status.pending = Some(item);
                cx.notify();
            });
            w.render_frame(cx);
            assert_eq!(w.find(format!("row-{item}")).bounds(), before);
            assert_eq!(w.find(format!("row-{other}")).bounds(), other_before);
            assert_eq!(
                w.find(format!("status-{item}")).label(),
                Some("Status for Ink: Saving…")
            );
            assert_eq!(
                w.find(format!("status-{other}")).label(),
                Some("Status for Sketch: Empty")
            );
            assert!(w.try_find("status-notice").is_none());
            table.update(cx, |table, cx| {
                let target = Target {
                    generation: table.status.generation,
                    column: Uuid::from_u128(500),
                    item,
                    revision: 0,
                };
                table.finish_status_write(
                    target,
                    "Done".into(),
                    Ok(StatusCell {
                        item_id: item,
                        revision: 1,
                        value: Some("done".into()),
                    }),
                    None,
                    cx,
                );
            });
            w.render_frame(cx);
            assert_eq!(w.find(format!("row-{item}")).bounds(), before);
            assert_eq!(
                w.find(format!("status-{item}")).label(),
                Some("Status for Ink: Done")
            );
            assert!(!table.read(cx).pending);
            assert!(!table.read(cx).status.is_saving());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn readonly_status_and_failed_reads_cannot_open_picker(cx: &mut TestAppContext) {
        let (handle, table) = visit_titles(cx, false, ["Ink", "Sketch"]);
        let trigger = format!("status-{}", Uuid::from_u128(59));
        cx.update_window(handle, |_, w, cx| {
            table.update(cx, |table, cx| {
                table.status.accept(accepted(Some("done"), 2), cx)
            });
            w.render_frame(cx);
            assert_eq!(w.find(trigger.clone()).disabled(), Some(true));
            w.click(trigger.clone(), cx);
            assert!(w.try_find("status-clear").is_none());
            table.update(cx, |table, cx| {
                table.finish_status_read(table.status.generation, Err(EditError::Transport), cx);
            });
            w.render_frame(cx);
            assert!(w.find("reload-status").visible());
            assert_eq!(w.find(trigger.clone()).disabled(), Some(true));
            assert_eq!(
                table.read(cx).status.accepted.as_ref().unwrap().cells[0]
                    .value
                    .as_deref(),
                Some("done")
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn status_read_generation_and_conflict_review_keep_entry_and_selection(
        cx: &mut TestAppContext,
    ) {
        let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
        let item = Uuid::from_u128(59);
        cx.update_window(handle, |_, w, cx| {
            table.update(cx, |table, cx| table.status.accept(accepted(None, 0), cx));
            w.render_frame(cx);
            w.click(format!("check-{item}"), cx);
            w.click("add-input-g-a", cx);
            w.input("Retained entry", cx);
            table.update(cx, |table, cx| {
                let stale = table.status.generation;
                table.status.generation = Uuid::new_v4();
                table.finish_status_read(stale, Ok(accepted(Some("working"), 1)), cx);
                assert!(
                    table.status.accepted.as_ref().unwrap().cells[0]
                        .value
                        .is_none()
                );
                let target = Target {
                    generation: table.status.generation,
                    column: Uuid::from_u128(500),
                    item,
                    revision: 0,
                };
                table.pending = true;
                table.finish_status_write(
                    target,
                    "Done".into(),
                    Err(EditError::Conflict),
                    Some(Ok(accepted(Some("stuck"), 1))),
                    cx,
                );
            });
            w.render_frame(cx);
            assert_eq!(w.find("add-input-g-a").value(), Some("Retained entry"));
            assert_eq!(w.find(format!("check-{item}")).checked(), Some(true));
            assert!(!table.read(cx).pending);
            assert!(!table.read(cx).status.failed);
            assert!(table.read(cx).status.request.is_none());
            assert_eq!(
                table.read(cx).status.accepted.as_ref().unwrap().cells[0]
                    .value
                    .as_deref(),
                Some("stuck")
            );
            assert!(
                table
                    .read(cx)
                    .status
                    .notice
                    .as_ref()
                    .unwrap()
                    .contains("choosing Done again")
            );
            w.click(format!("status-{item}"), cx);
            assert_eq!(w.find("status-choice-stuck").focused(), Some(true));
            w.press("escape", cx);
        })
        .unwrap();
    }
}
