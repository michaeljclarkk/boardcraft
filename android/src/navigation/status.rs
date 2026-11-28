use super::*;
use boardcraft_mobile_edit_client::{MainTableGroup, StatusCell, StatusChoice};
use gpui_kit::component::{Colorize, button::ButtonCustomVariant, popover::Popover};
use std::{collections::HashMap, sync::Arc};

mod selection;

pub(crate) struct StatusColumn {
    id: Uuid,
    pub(super) title: String,
    pub(super) choices: Arc<[StatusChoice]>,
    pub(super) cells: HashMap<Uuid, StatusCell>,
    pub(crate) editable: bool,
    pub(crate) pending: Option<Uuid>,
    selection_column: bool,
    selection_pending: bool,
    selection_errors: Vec<(String, &'static str)>,
    needs_reload: bool,
}

impl StatusColumn {
    /// The grouped board read supplies the current edit hint and row order.
    pub(super) fn load(
        client: &EditClient,
        board: Uuid,
    ) -> Result<(Vec<SavedItem>, Vec<MainTableGroup>, Self), EditError> {
        let board_rows = client.read_main_table(board)?;
        let table = client.read_status(board)?;
        let rows = board_rows
            .groups()
            .iter()
            .flat_map(|g| g.items())
            .cloned()
            .collect();
        Ok((
            rows,
            board_rows.groups().to_vec(),
            Self {
                id: table.column_id,
                title: table.title,
                choices: table.choices.into(),
                cells: table.cells.into_iter().map(|c| (c.item_id, c)).collect(),
                editable: board_rows.is_editable(),
                pending: None,
                selection_column: false,
                selection_pending: false,
                selection_errors: Vec::new(),
                needs_reload: false,
            },
        ))
    }

    pub(super) fn cell(
        &self,
        board: Uuid,
        row: &SavedItem,
        busy: bool,
        window: &mut Window,
        cx: &mut Context<BoardView>,
    ) -> AnyElement {
        let item = row.id;
        let Some(cell) = self.cells.get(&item) else {
            // Separate authorized snapshots may straddle an archive. Never invent a baseline.
            return div()
                .w(rems(8.75))
                .child(Label::new("Unavailable"))
                .into_any_element();
        };
        let selected = self
            .choices
            .iter()
            .find(|c| Some(&c.key) == cell.value.as_ref());
        let color = selected
            .and_then(|c| Hsla::parse_hex(&c.color).ok())
            .unwrap_or_else(empty_color);
        let label = selected.map(|c| c.label.as_str()).unwrap_or("");
        let disabled = busy || self.is_pending() || self.needs_reload || !self.editable;
        let trigger_focus = window
            .use_keyed_state(
                SharedString::from(format!("status-trigger-focus-{item}")),
                cx,
                |_, cx| cx.focus_handle().tab_stop(true),
            )
            .read(cx)
            .clone();
        let trigger = Button::new(SharedString::from(format!("status-{item}")))
            .large()
            .track_focus(&trigger_focus)
            .w(rems(8.75))
            .flex_shrink_0()
            .custom(tile_style(color, cx))
            .rounded_none()
            .bg(color)
            .text_color(gpui_kit::white())
            .label(if label.is_empty() { " " } else { label }.to_owned())
            .accessibility_label(format!(
                "{}: {} — {}",
                self.title,
                if label.is_empty() { "Empty" } else { label },
                row.title
            ))
            .loading(self.pending == Some(item))
            .disabled(disabled);
        if disabled {
            return trigger.into_any_element();
        }
        let target = Target {
            board,
            column: self.id,
            item,
            revision: cell.revision,
        };
        Picker {
            id: item.to_string(),
            choices: self.choices.clone(),
            current: cell.value.clone(),
            target: PickerTarget::Cell(target),
            trigger: trigger.on_mouse_down(MouseButton::Left, move |_, w, cx| {
                trigger_focus.focus(w, cx);
            }),
        }
        .render(window, cx)
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.pending.is_some() || self.selection_pending
    }
}

#[derive(Clone, Copy)]
enum PickerTarget {
    Cell(Target),
    Selection(SelectionTarget),
}

#[derive(Clone, Copy)]
struct SelectionTarget {
    board: Uuid,
    column: Uuid,
    visit: Uuid,
}

struct Picker {
    id: String,
    choices: Arc<[StatusChoice]>,
    current: Option<String>,
    target: PickerTarget,
    trigger: Button,
}

impl Picker {
    fn render(self, window: &mut Window, cx: &mut Context<BoardView>) -> AnyElement {
        let Self {
            id: item,
            choices,
            current,
            target,
            trigger,
        } = self;
        let selected_focus = window
            .use_keyed_state(
                SharedString::from(format!("status-selected-focus-{item}")),
                cx,
                |_, cx| cx.focus_handle().tab_stop(true),
            )
            .read(cx)
            .clone();
        let view = cx.weak_entity();
        Popover::new(SharedString::from(format!("status-picker-{item}")))
            .anchor(Anchor::TopRight)
            .track_focus(&selected_focus)
            .trigger(trigger)
            .content(move |_, w, cx| {
                let popup = cx.weak_entity();
                let button = |id: String, label: String, value: Option<String>| {
                    let owner = view.clone();
                    let popup = popup.clone();
                    Button::new(SharedString::from(id))
                        .large()
                        .label(label)
                        .on_click(move |_, w, cx| {
                            _ = popup.update(cx, |state, cx| state.dismiss(w, cx));
                            _ = owner.update(cx, |view, cx| match target {
                                PickerTarget::Cell(target) => {
                                    view.set_status(target, value.clone(), w, cx)
                                }
                                PickerTarget::Selection(target) => {
                                    view.set_selected_status(target, value.clone(), w, cx)
                                }
                            });
                        })
                };
                let panel_width = (w.viewport_size().width / w.rem_size() - 3.).clamp(16., 21.);
                let mut panel = div().flex().flex_col().gap_2().w(rems(panel_width));
                let mut grid_row = div().flex().gap_2();
                let mut handles = Vec::with_capacity(choices.len() + 1);
                for (ix, choice) in choices
                    .iter()
                    .map(Some)
                    .chain(std::iter::once(None))
                    .enumerate()
                {
                    let value = choice.map(|c| c.key.clone());
                    let label = choice.map(|c| c.label.as_str()).unwrap_or("Empty");
                    let color = choice
                        .and_then(|c| Hsla::parse_hex(&c.color).ok())
                        .unwrap_or_else(empty_color);
                    let checked = matches!(target, PickerTarget::Cell(_)) && current == value;
                    let marked = if checked {
                        format!("✓ {label}")
                    } else {
                        label.to_owned()
                    };
                    let id = match &value {
                        Some(key) => format!("status-option-{item}-{key}"),
                        None => format!("status-empty-{item}"),
                    };
                    let focus =
                        if checked || (matches!(target, PickerTarget::Selection(_)) && ix == 0) {
                            selected_focus.clone()
                        } else {
                            w.use_keyed_state(SharedString::from(id.clone()), cx, |_, cx| {
                                cx.focus_handle().tab_stop(true)
                            })
                            .read(cx)
                            .clone()
                        };
                    handles.push(focus.clone());
                    grid_row = grid_row.child(
                        button(id, marked, value)
                            .track_focus(&focus)
                            .flex_1()
                            .min_w_0()
                            .custom(tile_style(color, cx))
                            .bg(color)
                            .text_color(gpui_kit::white())
                            .toggled(checked),
                    );
                    if ix % 2 == 1 || ix == choices.len() {
                        if ix % 2 == 0 {
                            grid_row = grid_row.child(div().flex_1());
                        }
                        panel = panel.child(grid_row);
                        grid_row = div().flex().gap_2();
                    }
                }
                panel
                    .child(
                        button(format!("status-clear-{item}"), "Clear value".into(), None)
                            .ghost()
                            .w_full(),
                    )
                    .on_key_down(move |event: &KeyDownEvent, w, cx| {
                        let Some(ix) = handles.iter().position(|handle| handle.is_focused(w))
                        else {
                            return;
                        };
                        let last = handles.len() - 1;
                        let next = match event.keystroke.key.as_str() {
                            "down" => (ix + 1) % handles.len(),
                            "up" => (ix + last) % handles.len(),
                            "home" => 0,
                            "end" => last,
                            _ => return,
                        };
                        handles[next].focus(w, cx);
                        cx.stop_propagation();
                    })
            })
            .into_any_element()
    }
}

// Empty grey and white label ink are reference Status palette content, not
// application-theme surfaces. Preserve them alongside server-provided colours.
fn empty_color() -> Hsla {
    rgb(0xc4c4c4).into()
}

fn tile_style(color: Hsla, cx: &App) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(color)
        .foreground(gpui_kit::white())
        .hover(color.darken(0.08))
        .active(color.darken(0.15))
}

#[derive(Clone, Copy)]
struct Target {
    board: Uuid,
    column: Uuid,
    item: Uuid,
    revision: i64,
}

enum Outcome {
    Accepted(StatusCell),
    Reloaded(EditError, Vec<SavedItem>, Vec<MainTableGroup>, StatusColumn),
}

impl BoardView {
    fn set_status(
        &mut self,
        target: Target,
        value: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(conn), Some(client), Some(Page::Selected { board, status, .. })) =
            (self.connection, self.client.clone(), &mut self.navigation)
        else {
            return;
        };
        if *board != target.board
            || status.id != target.column
            || !status.editable
            || self.logging_out
            || self.automation_pending
            || self.editor.is_some()
            || status.is_pending()
            || status.needs_reload
            || !status
                .cells
                .get(&target.item)
                .is_some_and(|c| c.revision == target.revision && c.value != value)
        {
            return;
        }
        status.pending = Some(target.item);
        self.message = "";
        let identifier = Login::identifier(window, cx);
        let work = cx.background_executor().spawn(async move {
            match client.set_status(
                target.board,
                target.column,
                target.item,
                target.revision,
                value,
            ) {
                Ok(saved) => Ok(Outcome::Accepted(saved)),
                Err(EditError::SessionRejected) => Err(EditError::SessionRejected),
                Err(error) => StatusColumn::load(&client, target.board)
                    .map(|(rows, groups, status)| Outcome::Reloaded(error, rows, groups, status)),
            }
        });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            _ = view.update(cx, |view, cx| {
                let Some(Page::Selected {
                    board,
                    rows,
                    groups,
                    status,
                    ..
                }) = &mut view.navigation
                else {
                    return;
                };
                if *board != target.board
                    || status.id != target.column
                    || status.pending != Some(target.item)
                {
                    return;
                }
                match result {
                    Ok(Outcome::Accepted(cell)) => {
                        status.cells.insert(cell.item_id, cell);
                        status.pending = None;
                    }
                    Ok(Outcome::Reloaded(error, mut fresh_rows, fresh_groups, fresh_status)) => {
                        // A realtime name can advance while reconciliation is reading.
                        // Status recovery must never roll that accepted name back.
                        for fresh in &mut fresh_rows {
                            if let Some(current) = rows.iter().find(|row| row.id == fresh.id)
                                && current.revision > fresh.revision
                            {
                                *fresh = current.clone();
                            }
                        }
                        *rows = fresh_rows;
                        *groups = fresh_groups;
                        view.selected
                            .retain(|id| rows.iter().any(|row| row.id == *id));
                        *status = fresh_status;
                        view.message = match error {
                            EditError::Conflict => {
                                "Status changed elsewhere. Review the current value."
                            }
                            EditError::PermissionDenied => "No permission to change status.",
                            EditError::Missing => "Item or column no longer available.",
                            EditError::InvalidDraft => "Status choice unavailable. Choose again.",
                            _ => "Could not confirm the change. Current status reloaded.",
                        };
                    }
                    Err(EditError::SessionRejected) => view.forget_session(conn, identifier, cx),
                    Err(EditError::Missing | EditError::PermissionDenied) => {
                        view.navigation = Some(Page::Denied)
                    }
                    Err(_) => {
                        status.pending = None;
                        status.needs_reload = true;
                        view.message =
                            "Could not confirm status. Reload board before changing it again.";
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
