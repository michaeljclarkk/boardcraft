use super::*;
use boardcraft_mobile_edit_client::{MainTableGroup, StatusCell, StatusChoice};
use gpui_kit::component::{ActiveTheme, Colorize, popover::Popover};
use std::{collections::HashMap, sync::Arc};

pub(crate) struct StatusColumn {
    id: Uuid,
    pub(super) title: String,
    pub(super) choices: Arc<[StatusChoice]>,
    pub(super) cells: HashMap<Uuid, StatusCell>,
    pub(crate) editable: bool,
    pub(crate) pending: Option<Uuid>,
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
            },
        ))
    }

    pub(super) fn cell(
        &self,
        board: Uuid,
        row: &SavedItem,
        busy: bool,
        cx: &mut Context<BoardView>,
    ) -> AnyElement {
        let item = row.id;
        let Some(cell) = self.cells.get(&item) else {
            // Separate authorized snapshots may straddle an archive. Never invent a baseline.
            return div()
                .w_24()
                .child(Label::new("Unavailable"))
                .into_any_element();
        };
        let selected = self
            .choices
            .iter()
            .find(|c| Some(&c.key) == cell.value.as_ref());
        let color = selected
            .and_then(|c| Hsla::parse_hex(&c.color).ok())
            .unwrap_or(cx.theme().muted);
        let label = selected.map(|c| c.label.as_str()).unwrap_or("");
        let disabled = busy || self.pending.is_some() || !self.editable;
        let trigger = Button::new(SharedString::from(format!("status-{item}")))
            .large()
            .w_24()
            .flex_shrink_0()
            .bg(color)
            .text_color(if color.l < 0.45 {
                cx.theme().background
            } else {
                cx.theme().foreground
            })
            .label(label.to_owned())
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
        let choices = self.choices.clone();
        let current = cell.value.clone();
        let target = Target {
            board,
            column: self.id,
            item,
            revision: cell.revision,
        };
        let view = cx.weak_entity();
        Popover::new(SharedString::from(format!("status-picker-{item}")))
            .anchor(Anchor::TopRight)
            .trigger(trigger)
            .content(move |_, _, cx| {
                let button = |id: String, label: String, value: Option<String>| {
                    let owner = view.clone();
                    let popup = cx.weak_entity();
                    Button::new(SharedString::from(id))
                        .large()
                        .label(label)
                        .on_click(move |_, w, cx| {
                            _ = popup.update(cx, |state, cx| state.dismiss(w, cx));
                            _ = owner.update(cx, |view, cx| {
                                view.set_status(target, value.clone(), w, cx)
                            });
                        })
                };
                let mut panel = div().flex().flex_col().gap_2();
                let mut grid_row = div().flex().gap_2();
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
                        .unwrap_or(cx.theme().muted);
                    let marked = if current == value {
                        format!("✓ {label}")
                    } else {
                        label.to_owned()
                    };
                    let id = match &value {
                        Some(key) => format!("status-option-{item}-{key}"),
                        None => format!("status-empty-{item}"),
                    };
                    grid_row = grid_row.child(button(id, marked, value).w_32().bg(color));
                    if ix % 2 == 1 || ix == choices.len() {
                        panel = panel.child(grid_row);
                        grid_row = div().flex().gap_2();
                    }
                }
                if current.is_some() {
                    panel = panel.child(button(
                        format!("status-clear-{item}"),
                        "Clear value".into(),
                        None,
                    ));
                }
                panel
            })
            .into_any_element()
    }
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
            || status.pending.is_some()
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
                    Ok(Outcome::Reloaded(error, fresh_rows, fresh_groups, fresh_status)) => {
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
                        view.navigation = Some(Page::Fault);
                        view.message = "Could not reload status. Reopen the board to try again.";
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
