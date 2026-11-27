//! One dialog owns one ordered command. Review never sends a mutation.
use super::*;
use gpui_kit::component::{
    IndexPath, WindowExt,
    select::{Select, SelectItem, SelectState},
};

#[derive(Clone)]
pub(super) struct GroupChoice {
    pub(super) id: SharedString,
    pub(super) name: SharedString,
}
impl SelectItem for GroupChoice {
    type Value = SharedString;
    fn title(&self) -> SharedString {
        self.name.clone()
    }
    fn value(&self) -> &SharedString {
        &self.id
    }
}

struct Root {
    id: Uuid,
    name: SharedString,
    revision: i64,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Ready,
    Saving,
    Refused,
    Uncertain,
    Reading,
    Reviewed,
}

pub(super) struct MoveDialog {
    operation: Uuid,
    board_revision: i64,
    roots: Vec<Root>,
    destination: Entity<SelectState<Vec<GroupChoice>>>,
    confirm_focus: FocusHandle,
    state: State,
    notice: &'static str,
    review: Option<MainTableRead>,
}
enum MoveEvent {
    Confirm,
    Cancel,
    Review,
    Adopt,
}
impl EventEmitter<MoveEvent> for MoveDialog {}

impl MoveDialog {
    fn pending(&self) -> bool {
        matches!(self.state, State::Saving | State::Reading)
    }
    fn adoptable(&self) -> bool {
        self.review.as_ref().is_some_and(|read| {
            read.is_editable()
                && self.roots.iter().all(|root| {
                    read.groups()
                        .iter()
                        .flat_map(|g| g.items())
                        .any(|r| r.id == root.id)
                })
        })
    }
    fn emit(&self, event: MoveEvent, cx: &mut Context<Self>) {
        if !self.pending() {
            cx.emit(event);
        }
    }
}

impl Render for MoveDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new(format!("{} selected", self.roots.len())))
            .child(Label::new(self.notice))
            .child(Label::new(
                self.roots
                    .iter()
                    .take(6)
                    .map(|r| r.name.as_ref())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        if self.roots.len() > 6 {
            body = body.child(Label::new(format!("and {} more", self.roots.len() - 6)));
        }
        body = body
            .child(Label::new("Destination board"))
            .child(
                Button::new("move-board")
                    .label("Current board")
                    .disabled(true),
            )
            .child(Label::new("Destination group"))
            .child(
                div().h_8().child(
                    Select::new(&self.destination)
                        .accessibility_label("Destination group")
                        .disabled(self.pending()),
                ),
            );
        if let Some(read) = &self.review {
            body = body.child(Label::new(format!(
                "Current board revision {}",
                read.revision()
            )));
            for root in &self.roots {
                let current = read
                    .groups()
                    .iter()
                    .flat_map(|g| g.items())
                    .find(|r| r.id == root.id);
                body = body.child(Label::new(match current {
                    Some(r) => format!(
                        "{} · {} · revision {} (requested {})",
                        r.title, root.id, r.revision, root.revision
                    ),
                    None => format!(
                        "{} · unavailable (requested revision {})",
                        root.id, root.revision
                    ),
                }));
            }
            let group_present = self
                .destination
                .read(cx)
                .selected_value()
                .is_some_and(|id| read.groups().iter().any(|g| g.id() == id.as_ref()));
            body = body.child(
                Button::new("move-adopt")
                    .label("Use these revisions and keep my choices")
                    .disabled(!self.adoptable() || !group_present)
                    .on_click(cx.listener(|d, _, _, cx| d.emit(MoveEvent::Adopt, cx))),
            );
        }
        if matches!(
            self.state,
            State::Refused | State::Uncertain | State::Reviewed | State::Reading
        ) {
            body = body.child(
                Button::new("move-review")
                    .label("Review latest items")
                    .disabled(self.pending())
                    .on_click(cx.listener(|d, _, _, cx| d.emit(MoveEvent::Review, cx))),
            );
        }
        body.child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("move-cancel")
                        .label("Cancel")
                        .disabled(self.pending())
                        .on_click(cx.listener(|d, _, _, cx| d.emit(MoveEvent::Cancel, cx))),
                )
                .child(
                    Button::new("move-confirm")
                        .track_focus(&self.confirm_focus)
                        .primary()
                        .label("Confirm")
                        .loading(self.state == State::Saving)
                        .disabled(self.state != State::Ready)
                        .on_click(cx.listener(|d, _, _, cx| d.emit(MoveEvent::Confirm, cx))),
                ),
        )
    }
}

enum Failure {
    Save(EditError),
    Read(EditError),
}

impl Connected {
    #[cfg(all(windows, feature = "native-smoke"))]
    pub(crate) fn selection_move_snapshot(
        &self,
        cx: &App,
    ) -> Option<(Uuid, SharedString, Vec<Uuid>)> {
        let Screen::Navigation(nav) = &self.screen else {
            return None;
        };
        let Page::Items(_, table) = &nav.page else {
            return None;
        };
        let dialog = table.move_dialog.as_ref()?.read(cx);
        Some((
            dialog.operation,
            dialog.destination.read(cx).selected_value()?.clone(),
            dialog.roots.iter().map(|r| r.id).collect(),
        ))
    }

    pub(super) fn open_selection_move(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(_, table) = &mut nav.page else {
            return;
        };
        if table.pending
            || table.reconcile
            || !table.editable
            || table.selection.ids.is_empty()
            || table.move_dialog.is_some()
            || table.item_move.is_some()
        {
            return;
        }
        let choices: Vec<_> = table
            .headings
            .iter()
            .map(|h| GroupChoice {
                id: h.id.clone(),
                name: h.name.clone(),
            })
            .collect();
        let destination = cx.new(|cx| SelectState::new(choices, Some(IndexPath::default()), w, cx));
        let roots = table
            .selection
            .ids
            .iter()
            .map(|id| {
                table
                    .headings
                    .iter()
                    .flat_map(|h| &h.rows)
                    .find(|r| r.read(cx).edit.identity().1.as_uuid() == *id)
                    .map(|r| {
                        let r = r.read(cx);
                        Root {
                            id: *id,
                            name: r.edit.accepted().to_owned().into(),
                            revision: r.edit.revision(),
                        }
                    })
            })
            .collect::<Option<Vec<_>>>();
        let Some(roots) = roots else {
            self.message = "Selected item unavailable. Reload board before moving items.";
            cx.notify();
            return;
        };
        let dialog = cx.new(|cx| MoveDialog {
            operation: Uuid::new_v4(),
            board_revision: table.board_revision,
            roots,
            destination,
            confirm_focus: cx.focus_handle().tab_stop(true),
            state: State::Ready,
            notice: "",
            review: None,
        });
        table._move_subscription =
            Some(
                cx.subscribe_in(&dialog, w, |v, d, event: &MoveEvent, w, cx| {
                    v.selection_move_event(d.clone(), event, w, cx);
                }),
            );
        table.move_dialog = Some(dialog.clone());
        table.move_focus.focus(w, cx);
        let cancel = dialog.downgrade();
        let confirm = cancel.clone();
        w.open_dialog(cx, move |surface, _, cx| {
            let busy = dialog.read(cx).pending();
            let cancel = cancel.clone();
            let confirm = confirm.clone();
            surface
                .title("Move selected items")
                .overlay_closable(false)
                .close_button(!busy)
                .on_cancel(move |_, _, cx| {
                    _ = cancel.update(cx, |d, cx| d.emit(MoveEvent::Cancel, cx));
                    false
                })
                // Dialog's Enter action also bubbles from Select. Only a focused
                // Confirm control is a deliberate keyboard submission.
                .on_ok(move |_, w, cx| {
                    _ = confirm.update(cx, |d, cx| {
                        if d.confirm_focus.is_focused(w) {
                            d.emit(MoveEvent::Confirm, cx);
                        }
                    });
                    false
                })
                .child(dialog.clone())
        });
        cx.notify();
    }

    fn selection_move_event(
        &mut self,
        dialog: Entity<MoveDialog>,
        event: &MoveEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, table) = &mut nav.page else {
            return;
        };
        if table.pending || table.move_dialog.as_ref() != Some(&dialog) {
            return;
        }
        match event {
            MoveEvent::Cancel => {
                table.move_dialog = None;
                table._move_subscription = None;
                w.close_dialog(cx);
                cx.notify();
                return;
            }
            MoveEvent::Adopt => {
                dialog.update(cx, |d, cx| {
                    if d.state != State::Reviewed || !d.adoptable() {
                        return;
                    }
                    let read = d.review.as_ref().unwrap();
                    if !d
                        .destination
                        .read(cx)
                        .selected_value()
                        .is_some_and(|id| read.groups().iter().any(|g| g.id() == id.as_ref()))
                    {
                        return;
                    }
                    d.board_revision = read.revision();
                    for root in &mut d.roots {
                        let r = read
                            .groups()
                            .iter()
                            .flat_map(|g| g.items())
                            .find(|r| r.id == root.id)
                            .unwrap();
                        root.revision = r.revision;
                        root.name = r.title.clone().into();
                    }
                    d.review = None;
                    d.state = State::Ready;
                    d.notice = "Revisions adopted. Confirm separately to move the selected items.";
                    cx.notify();
                });
                return;
            }
            MoveEvent::Confirm
                if dialog.read(cx).state != State::Ready || !table.editable || table.reconcile =>
            {
                return;
            }
            _ => (),
        }
        let board = *board;
        let visit = self.navigation_request;
        let client = nav.client.clone();
        let reviewing = matches!(event, MoveEvent::Review);
        let uncertain = dialog.read(cx).state == State::Uncertain;
        let d = dialog.read(cx);
        let Some(group) = d.destination.read(cx).selected_value() else {
            return;
        };
        let command = (
            d.operation,
            group.to_string(),
            d.board_revision,
            d.roots
                .iter()
                .map(|r| (r.id, r.revision))
                .collect::<Vec<_>>(),
        );
        table.set_busy(true, cx);
        dialog.update(cx, |d, cx| {
            d.state = if reviewing {
                State::Reading
            } else {
                State::Saving
            };
            d.notice = if reviewing {
                "Reading current items…"
            } else {
                "Moving selected items…"
            };
            cx.notify();
        });
        #[cfg(all(windows, feature = "native-smoke"))]
        if !reviewing {
            self.submissions += 1;
        }
        let work = cx.background_executor().spawn(async move {
            if !reviewing {
                client
                    .move_main_table_selection(board, command.0, &command.1, command.2, &command.3)
                    .map_err(Failure::Save)?;
            }
            client.read_main_table(board).map_err(Failure::Read)
        });
        cx.spawn_in(w, async move |v, cx| {
            let result = work.await;
            _ = v.update_in(cx, |v, w, cx| {
                if v.navigation_request != visit { return; }
                let Screen::Navigation(nav) = &mut v.screen else { return; };
                let Page::Items(current, old) = &mut nav.page else { return; };
                if *current != board || old.move_dialog.as_ref() != Some(&dialog) { return; }
                old.set_busy(false, cx);
                match result {
                    Ok(read) if reviewing && !uncertain && read.is_editable() => {
                        dialog.update(cx, |d, cx| {
                            d.review = Some(read);
                            d.state = State::Reviewed;
                            d.notice = "Review every requested item before adopting these revisions.";
                            cx.notify();
                        });
                    }
                    Ok(read) => {
                        let mut table = MainTable::new(read, board, Some(old), w, cx);
                        if !reviewing || uncertain { table.selection = Selection::default(); }
                        w.close_dialog(cx);
                        // The toolbar disappears after success. Restore to a live
                        // disclosure only after Kit has restored the dialog trigger.
                        table.focus_first(w, cx);
                        nav.page = Page::Items(board, Box::new(table));
                        v.message = if uncertain { "Board reloaded. Review accepted items before another move." }
                            else if reviewing { "Move not saved: permission denied." } else { "" };
                    }
                    Err(Failure::Save(EditError::SessionRejected) | Failure::Read(EditError::SessionRejected)) => {
                        w.close_dialog(cx);
                        v.link_input = None;
                        v.screen = Self::identifier(w, cx);
                        v.message = "Session expired. Sign in again.";
                    }
                    Err(Failure::Read(EditError::PermissionDenied | EditError::Missing)) => {
                        w.close_dialog(cx);
                        nav.page = Page::Unavailable;
                        nav.share = None;
                        v.message = "Destination unavailable";
                    }
                    Err(failure) => {
                        let denied = matches!(failure, Failure::Save(EditError::PermissionDenied));
                        dialog.update(cx, |d, cx| {
                            d.review = None;
                            d.state = if uncertain || matches!(failure, Failure::Read(_) | Failure::Save(EditError::Transport | EditError::AccessUnavailable | EditError::HostFault)) { State::Uncertain } else { State::Refused };
                            d.notice = if denied { "Move not saved: permission denied. Checking current access…" }
                                else if d.state == State::Uncertain { "Move not confirmed. Read the board before another move; this request will not be repeated." }
                                else { "Items changed or are unavailable. Review latest items before confirming again." };
                            cx.notify();
                        });
                        // Closing an uncertain dialog must not bypass the read barrier.
                        if dialog.read(cx).state == State::Uncertain {
                            old.reconcile = true;
                            old.set_busy(false, cx);
                        }
                        if denied { v.selection_move_event(dialog.clone(), &MoveEvent::Review, w, cx); }
                    }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}
