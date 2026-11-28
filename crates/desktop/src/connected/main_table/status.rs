//! One authorized palette read per table load. A picker captures the cell revision
//! it displayed; only the host's reply can change the accepted value.
use super::*;
use boardcraft_mobile_edit_client::{StatusCell, StatusChoice, StatusTable};
use gpui_kit::component::{
    Colorize,
    button::ButtonCustomVariant,
    popover::{Popover, PopoverState},
};
use gpui_kit::prelude::FluentBuilder;
use std::{collections::HashMap, rc::Rc};

pub(super) struct StatusColumn {
    // A board can be reloaded or revisited while its old read is still running.
    visit: Uuid,
    read: ColumnRead,
    requires_reload: bool,
    pending: Option<Uuid>,
    notice: Option<(Uuid, String)>,
}

enum ColumnRead {
    Loading,
    Failed,
    Ready {
        column: Uuid,
        title: String,
        choices: Rc<[StatusChoice]>,
        cells: HashMap<Uuid, CellView>,
    },
}

struct CellView {
    saved: StatusCell,
    trigger: FocusHandle,
    selected: FocusHandle,
}

impl StatusColumn {
    pub(super) fn load(board: Uuid, w: &Window, cx: &mut Context<Connected>) -> Self {
        let visit = Uuid::new_v4();
        // MainTable::new is also used for reload, insert, restore and move.
        // Start after the replacement has been installed as the current page.
        cx.defer_in(w, move |view, w, cx| view.read_status(board, visit, w, cx));
        Self {
            visit,
            read: ColumnRead::Loading,
            requires_reload: false,
            pending: None,
            notice: None,
        }
    }

    fn accept(&mut self, read: StatusTable, cx: &mut App) {
        self.requires_reload = false;
        let cells = read
            .cells
            .into_iter()
            .map(|cell| {
                let previous = match &self.read {
                    ColumnRead::Ready { cells, .. } => cells.get(&cell.item_id),
                    _ => None,
                };
                (
                    cell.item_id,
                    CellView {
                        trigger: previous
                            .map(|c| c.trigger.clone())
                            .unwrap_or_else(|| cx.focus_handle().tab_stop(true)),
                        selected: cx.focus_handle().tab_stop(true),
                        saved: cell,
                    },
                )
            })
            .collect();
        self.read = ColumnRead::Ready {
            column: read.column_id,
            title: read.title,
            choices: read.choices.into(),
            cells,
        };
    }

    pub(super) fn header(&self) -> impl IntoElement {
        let title = match &self.read {
            ColumnRead::Ready { title, .. } => title,
            _ => "Status",
        };
        lane().child(Label::new(title.to_owned()))
    }

    pub(super) fn notice_for(&self, item: Uuid) -> Option<String> {
        if self.pending == Some(item) {
            return None;
        }
        self.notice
            .as_ref()
            .filter(|(id, _)| *id == item)
            .map(|(_, text)| text.clone())
    }
}

fn lane() -> Div {
    div()
        .w_40()
        .h_full()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
}

fn color(choice: &StatusChoice, cx: &App) -> Hsla {
    Hsla::parse_hex(&choice.color).unwrap_or(cx.theme().muted_foreground)
}

#[derive(Clone, Copy)]
struct Target {
    visit: Uuid,
    column: Uuid,
    item: Uuid,
    revision: i64,
}

impl MainTable {
    pub(super) fn status_cell(
        &self,
        id: Uuid,
        name: &str,
        cx: &mut Context<Connected>,
    ) -> AnyElement {
        let ColumnRead::Ready {
            column,
            title,
            choices,
            cells,
        } = &self.status.read
        else {
            return lane()
                .child(Label::new(
                    if matches!(self.status.read, ColumnRead::Loading) {
                        "Loading…"
                    } else {
                        "Reload board"
                    },
                ))
                .into_any_element();
        };
        let Some(cell) = cells.get(&id) else {
            return lane().child(Label::new("Reload board")).into_any_element();
        };
        let selected = choices
            .iter()
            .find(|c| Some(&c.key) == cell.saved.value.as_ref());
        // The palette (including the reference UI's empty grey) is cell content.
        let pending = self.status.pending == Some(id);
        let bg = if pending {
            // The source's saving prompt is cell content, not a table notice.
            rgb(0x757575).into()
        } else {
            selected.map_or_else(|| rgb(0xc4c4c4).into(), |c| color(c, cx))
        };
        let label = if pending {
            Some("Saving…")
        } else {
            selected.map(|c| c.label.as_str())
        };
        let button = Button::new(SharedString::from(format!("status-{id}")))
            .track_focus(&cell.trigger)
            .custom(tile_style(bg, cx))
            .rounded_none()
            .bg(bg)
            .text_color(gpui_kit::white())
            .w_40()
            .h_full()
            .label(label.unwrap_or(" ").to_owned())
            .accessibility_label(format!("{title} for {name}: {}", label.unwrap_or("Empty")));
        if !self.editable || self.pending || self.reconcile || self.status.requires_reload {
            return lane().child(button.disabled(true)).into_any_element();
        }
        let target = Target {
            visit: self.status.visit,
            column: *column,
            item: id,
            revision: cell.saved.revision,
        };
        let view = cx.weak_entity();
        let choices = Rc::clone(choices);
        let value = cell.saved.value.clone();
        let trigger = cell.trigger.clone();
        let initial = cell.selected.clone();
        let chooser_label = format!("Choose {title}");
        let menu = Popover::new(SharedString::from(format!("status-popup-{id}")))
            .trigger_style(StyleRefinement::default().w_full().h_full())
            .track_focus(&initial)
            .trigger(button.on_mouse_down(MouseButton::Left, move |_, w, cx| trigger.focus(w, cx)))
            .content(move |_, w, cx| {
                let popup = cx.weak_entity();
                let commit = |key: Option<String>| {
                    let view = view.clone();
                    let popup = popup.clone();
                    move |_: &ClickEvent, w: &mut Window, cx: &mut App| {
                        _ = popup.update(cx, |p: &mut PopoverState, cx| p.dismiss(w, cx));
                        _ = view.update(cx, |v, cx| v.set_status(target, key.clone(), w, cx));
                    }
                };
                // Together with Kit's 12px surface inset this is the source's
                // 360px picker; both tile columns share the available width.
                let mut panel = div().flex().flex_col().gap_2().w(rems(21.));
                let mut options = div()
                    .id("status-options")
                    .role(Role::ListBox)
                    .aria_label(chooser_label.clone())
                    .flex()
                    .flex_col()
                    .gap_2();
                let mut row = div().flex().gap_2();
                let mut handles = Vec::with_capacity(choices.len() + 1);
                for (ix, option) in choices
                    .iter()
                    .map(Some)
                    .chain(std::iter::once(None))
                    .enumerate()
                {
                    let key = option.map(|o| o.key.clone());
                    let checked = value == key;
                    let id: SharedString = match option {
                        Some(o) => format!("status-choice-{}", o.key).into(),
                        None => "status-empty".into(),
                    };
                    let handle = if checked {
                        initial.clone()
                    } else {
                        w.use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle().tab_stop(true))
                            .read(cx)
                            .clone()
                    };
                    let bg = option.map_or_else(|| rgb(0xc4c4c4).into(), |c| color(c, cx));
                    let tile = Button::new(id)
                        .flex_1()
                        .min_w_0()
                        .h_8()
                        .rounded_sm()
                        .role(Role::ListBoxOption)
                        .track_focus(&handle)
                        .label(option.map_or("Empty", |c| c.label.as_str()).to_owned())
                        .custom(tile_style(bg, cx))
                        .bg(bg)
                        .text_color(gpui_kit::white())
                        .toggled(checked)
                        .when(checked, |b| b.icon(IconName::Check))
                        .on_click(commit(key));
                    handles.push(handle);
                    row = row.child(tile);
                    if ix % 2 == 1 || ix == choices.len() {
                        options = options.child(row);
                        row = div().flex().gap_2();
                    }
                }
                panel = panel.child(options).child(
                    Button::new("status-clear")
                        .w_full()
                        .h_8()
                        .bg(cx.theme().muted)
                        .label("Clear value")
                        .on_click(commit(None)),
                );
                panel.on_key_down(move |event: &KeyDownEvent, w, cx| {
                    let Some(ix) = handles.iter().position(|h| h.is_focused(w)) else {
                        return;
                    };
                    let n = handles.len();
                    let next = match event.keystroke.key.as_str() {
                        "down" => (ix + 1) % n,
                        "up" => (ix + n - 1) % n,
                        "home" => 0,
                        "end" => n - 1,
                        _ => return,
                    };
                    handles[next].focus(w, cx);
                    cx.stop_propagation();
                })
            });
        lane().child(menu).into_any_element()
    }
}

fn tile_style(bg: Hsla, cx: &App) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(bg)
        .foreground(gpui_kit::white())
        .hover(bg.darken(0.08))
        .active(bg.darken(0.15))
}

impl Connected {
    fn read_status(&mut self, board: Uuid, visit: Uuid, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        if !matches!(&nav.page, Page::Items(current, t) if *current == board && t.status.visit == visit)
        {
            return;
        }
        let client = nav.client.clone();
        let work = cx
            .background_executor()
            .spawn(async move { client.read_status(board) });
        cx.spawn_in(w, async move |v, cx| {
            let result = work.await;
            _ = v.update_in(cx, |v, w, cx| v.finish_status_read(visit, result, w, cx));
        })
        .detach();
    }

    fn finish_status_read(
        &mut self,
        visit: Uuid,
        result: Result<StatusTable, EditError>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(_, t) = &mut nav.page else {
            return;
        };
        if t.status.visit != visit {
            return;
        }
        match result {
            Ok(table) => {
                t.status.accept(table, cx);
                if self.message == "Could not load statuses. Reload board to try again." {
                    self.message = "";
                }
            }
            Err(EditError::SessionRejected) => {
                self.stop_changes();
                self.link_input = None;
                self.screen = Self::identifier(w, cx);
                self.message = "Session expired. Sign in again.";
            }
            Err(EditError::PermissionDenied) => {
                nav.page = Page::Unavailable;
                nav.share = None;
                self.message = "Destination unavailable";
            }
            Err(_) => {
                // Keep an accepted cell visible after an unavailable recovery
                // read, but require a deliberate reload before another write.
                t.status.requires_reload = true;
                if !matches!(t.status.read, ColumnRead::Ready { .. }) {
                    t.status.read = ColumnRead::Failed;
                }
                self.message = "Could not load statuses. Reload board to try again.";
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
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, t) = &mut nav.page else {
            return;
        };
        if t.status.visit != target.visit
            || !t.editable
            || t.pending
            || t.reconcile
            || t.status.requires_reload
        {
            return;
        }
        let ColumnRead::Ready { choices, cells, .. } = &t.status.read else {
            return;
        };
        let Some(cell) = cells.get(&target.item) else {
            return;
        };
        cell.trigger.focus(w, cx);
        if cell.saved.value == value {
            return;
        }
        let draft = choices
            .iter()
            .find(|c| Some(&c.key) == value.as_ref())
            .map_or("Clear", |c| c.label.as_str())
            .to_owned();
        let board = *board;
        let client = nav.client.clone();
        t.status.pending = Some(target.item);
        t.status.notice = None;
        t.set_busy(true, cx);
        let work = cx.background_executor().spawn(async move {
            match client.set_status(board, target.column, target.item, target.revision, value) {
                Ok(saved) => Write::Accepted(saved),
                Err(EditError::SessionRejected) => Write::Expired,
                Err(EditError::PermissionDenied) => Write::Denied,
                Err(error) => Write::Review(error, client.read_status(board)),
            }
        });
        cx.spawn_in(w, async move |v, cx| {
            let result = work.await;
            _ = v.update_in(cx, |v, w, cx| {
                let Screen::Navigation(nav) = &mut v.screen else {
                    return;
                };
                let Page::Items(_, t) = &mut nav.page else {
                    return;
                };
                if t.status.visit != target.visit {
                    return;
                }
                t.status.pending = None;
                t.set_busy(false, cx);
                match result {
                    Write::Accepted(saved) => {
                        if let ColumnRead::Ready { cells, .. } = &mut t.status.read
                            && let Some(cell) = cells.get_mut(&saved.item_id)
                        {
                            cell.saved = saved;
                        }
                        t.status.notice = None;
                    }
                    Write::Expired => {
                        v.finish_status_read(target.visit, Err(EditError::SessionRejected), w, cx)
                    }
                    Write::Denied => {
                        let focus = cx.focus_handle().tab_stop(true);
                        focus.focus(w, cx);
                        nav.page = Page::Recover(board, focus);
                        nav.share = None;
                        v.message =
                            "Status not saved: permission denied. Reload board to check access.";
                    }
                    Write::Review(error, read) => {
                        let reason = if error == EditError::Conflict {
                            "Status changed elsewhere"
                        } else {
                            "Status not confirmed"
                        };
                        t.status.notice = Some((
                            target.item,
                            format!(
                                "{reason}. Review the current value before choosing {draft} again."
                            ),
                        ));
                        v.finish_status_read(target.visit, read, w, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

enum Write {
    Accepted(StatusCell),
    Expired,
    Denied,
    Review(EditError, Result<StatusTable, EditError>),
}

#[cfg(all(test, feature = "native-smoke"))]
mod tests;
