//! A row-menu group-field save is not the toolbar's ordered append command.
use super::*;
use gpui_kit::component::{
    IndexPath, WindowExt,
    select::{Select, SelectState},
};
use selection_move::GroupChoice;

#[derive(Clone)]
pub(super) struct FailedMove {
    id: Uuid,
    group: SharedString,
    review: Option<Review>,
}
#[derive(Clone)]
struct Review {
    source: SharedString,
    destination: SharedString,
    revision: Option<i64>,
}

pub(super) struct ItemMoveDialog {
    id: Uuid,
    revision: i64,
    changed_source: bool,
    destination: Entity<SelectState<Vec<GroupChoice>>>,
    pending: bool,
    confirm_focus: FocusHandle,
}
enum MoveEvent {
    Confirm,
    Cancel,
}
impl EventEmitter<MoveEvent> for ItemMoveDialog {}
impl ItemMoveDialog {
    fn emit(&self, event: MoveEvent, cx: &mut Context<Self>) {
        if !self.pending {
            cx.emit(event);
        }
    }
}
impl Render for ItemMoveDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new(format!("Item {}", self.id)))
            .child(Label::new("Destination board"))
            .child(
                Button::new("item-move-board")
                    .label("Current board")
                    .disabled(true),
            )
            .child(Label::new("Destination group"))
            .child(
                Select::new(&self.destination)
                    .accessibility_label("Destination group")
                    .disabled(self.pending),
            )
            .child(Label::new(if self.pending { "Saving…" } else { "" }))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("item-move-cancel")
                            .label("Cancel")
                            .disabled(self.pending)
                            .on_click(cx.listener(|d, _, _, cx| d.emit(MoveEvent::Cancel, cx))),
                    )
                    .child(
                        Button::new("item-move-confirm")
                            .primary()
                            .label("Confirm")
                            .track_focus(&self.confirm_focus)
                            .loading(self.pending)
                            .disabled(self.pending)
                            .on_click(cx.listener(|d, _, _, cx| d.emit(MoveEvent::Confirm, cx))),
                    ),
            )
    }
}

impl MainTable {
    pub(super) fn failed_item_moves(&self, cx: &mut Context<Connected>) -> impl IntoElement {
        let mut body = div().flex().flex_col().gap_2();
        for failed in &self.failed_moves {
            let id = failed.id;
            body = body.child(Label::new(format!("Move for {id} not confirmed.")));
            if let Some(review) = &failed.review {
                body = body
                    .child(Label::new(review.source.clone()))
                    .child(Label::new(review.destination.clone()));
                if let Some(revision) = review.revision {
                    body = body.child(
                        Button::new(SharedString::from(format!("item-move-apply-{id}")))
                            .label("Apply reviewed move")
                            .disabled(self.pending || self.reconcile || !self.editable)
                            .on_click(cx.listener(move |v, _, w, cx| {
                                v.apply_item_move(id, revision, w, cx)
                            })),
                    );
                }
            }
            body = body
                .child(
                    Button::new(SharedString::from(format!("item-move-review-{id}")))
                        .label("Review move")
                        .disabled(self.pending)
                        .on_click(cx.listener(move |v, _, w, cx| v.review_item_move(id, w, cx))),
                )
                .child(
                    Button::new(SharedString::from(format!("item-move-dismiss-{id}")))
                        .label("Dismiss")
                        .disabled(self.pending)
                        .on_click(cx.listener(move |v, _, _, cx| {
                            if let Screen::Navigation(nav) = &mut v.screen
                                && let Page::Items(_, t) = &mut nav.page
                            {
                                t.failed_moves.retain(|f| f.id != id);
                                cx.notify();
                            }
                        })),
                );
        }
        body
    }
    fn remember_move(&mut self, id: Uuid, group: SharedString) {
        self.failed_moves.retain(|f| f.id != id);
        self.failed_moves.push(FailedMove {
            id,
            group,
            review: None,
        });
    }
}

enum ReadIntent {
    Open {
        id: Uuid,
        displayed_group: SharedString,
    },
    Review(Uuid),
}
enum Failure {
    Save(EditError),
    Read(EditError),
}

impl Connected {
    #[cfg(all(windows, feature = "native-smoke"))]
    pub(crate) fn single_move_snapshot(&self, cx: &App) -> Option<(Uuid, SharedString, i64)> {
        let Screen::Navigation(nav) = &self.screen else {
            return None;
        };
        let Page::Items(_, t) = &nav.page else {
            return None;
        };
        let d = t.item_move.as_ref()?.read(cx);
        Some((
            d.id,
            d.destination.read(cx).selected_value()?.clone(),
            d.revision,
        ))
    }
    pub(super) fn open_item_move(&mut self, id: Uuid, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        let Page::Items(_, t) = &nav.page else {
            return;
        };
        if t.pending
            || t.reconcile
            || !t.editable
            || t.item_move.is_some()
            || t.move_dialog.is_some()
        {
            return;
        }
        let Some(h) = t.headings.iter().find(|h| {
            h.rows
                .iter()
                .any(|r| r.read(cx).edit.identity().1.as_uuid() == id)
        }) else {
            return;
        };
        let displayed_group = h.id.clone();
        self.read_item_move(
            ReadIntent::Open {
                id,
                displayed_group,
            },
            w,
            cx,
        );
    }
    fn review_item_move(&mut self, id: Uuid, w: &mut Window, cx: &mut Context<Self>) {
        self.read_item_move(ReadIntent::Review(id), w, cx);
    }
    fn read_item_move(&mut self, intent: ReadIntent, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, t) = &mut nav.page else {
            return;
        };
        if t.pending {
            return;
        }
        let board = *board;
        let visit = self.navigation_request;
        let client = nav.client.clone();
        t.set_busy(true, cx);
        self.message = "Reading current item placement…";
        let work = cx
            .background_executor()
            .spawn(async move { client.read_main_table(board) });
        cx.spawn_in(w, async move |v, cx| {
            let result = work.await;
            _ = v.update_in(cx, |v, w, cx| {
                if v.navigation_request != visit {
                    return;
                }
                let Screen::Navigation(nav) = &mut v.screen else {
                    return;
                };
                let Page::Items(current, old) = &mut nav.page else {
                    return;
                };
                if *current != board || !old.pending {
                    return;
                }
                match result {
                    Ok(read) => {
                        let mut t = MainTable::new(read, board, Some(old), w, cx);
                        let mut opening = None;
                        match intent {
                            ReadIntent::Open {
                                id,
                                displayed_group,
                            } if t.editable => {
                                if let Some(h) = t.headings.iter().find(|h| {
                                    h.rows
                                        .iter()
                                        .any(|r| r.read(cx).edit.identity().1.as_uuid() == id)
                                }) {
                                    let revision = h
                                        .rows
                                        .iter()
                                        .find(|r| r.read(cx).edit.identity().1.as_uuid() == id)
                                        .unwrap()
                                        .read(cx)
                                        .edit
                                        .revision();
                                    let changed_source = h.id != displayed_group;
                                    let choices: Vec<_> = t
                                        .headings
                                        .iter()
                                        .map(|h| GroupChoice {
                                            id: h.id.clone(),
                                            name: h.name.clone(),
                                        })
                                        .collect();
                                    let destination = cx.new(|cx| {
                                        SelectState::new(choices, Some(IndexPath::default()), w, cx)
                                    });
                                    let dialog = cx.new(|cx| ItemMoveDialog {
                                        id,
                                        revision,
                                        changed_source,
                                        destination,
                                        pending: false,
                                        confirm_focus: cx.focus_handle().tab_stop(true),
                                    });
                                    t._item_move_subscription = Some(cx.subscribe_in(
                                        &dialog,
                                        w,
                                        |v, d, e: &MoveEvent, w, cx| {
                                            v.item_move_event(d.clone(), e, w, cx)
                                        },
                                    ));
                                    t.item_move = Some(dialog.clone());
                                    if let Some(trigger) = t.archive_triggers.get(&id) {
                                        trigger.focus(w, cx);
                                    }
                                    opening = Some(dialog);
                                }
                            }
                            ReadIntent::Review(id) => {
                                if let Some(f) = t.failed_moves.iter_mut().find(|f| f.id == id) {
                                    let source = t.headings.iter().find(|h| {
                                        h.rows
                                            .iter()
                                            .any(|r| r.read(cx).edit.identity().1.as_uuid() == id)
                                    });
                                    let destination = t.headings.iter().find(|h| h.id == f.group);
                                    let revision = if t.editable && destination.is_some() {
                                        source
                                            .and_then(|h| {
                                                h.rows.iter().find(|r| {
                                                    r.read(cx).edit.identity().1.as_uuid() == id
                                                })
                                            })
                                            .map(|r| r.read(cx).edit.revision())
                                    } else {
                                        None
                                    };
                                    f.review = Some(Review {
                                        source: source
                                            .map(|h| {
                                                format!("Current group: {} ({})", h.name, h.id)
                                            })
                                            .unwrap_or_else(|| "Current item unavailable".into())
                                            .into(),
                                        destination: destination
                                            .map(|h| {
                                                format!("Requested group: {} ({})", h.name, h.id)
                                            })
                                            .unwrap_or_else(|| {
                                                format!("Requested group unavailable ({})", f.group)
                                            })
                                            .into(),
                                        revision,
                                    });
                                }
                            }
                            _ => (),
                        }
                        nav.page = Page::Items(board, Box::new(t));
                        v.message = if opening.is_some() {
                            ""
                        } else {
                            "Current placement loaded. A read does not repeat the move."
                        };
                        if let Some(dialog) = opening {
                            let cancel = dialog.downgrade();
                            let confirm = cancel.clone();
                            w.open_dialog(cx, move |surface, _, cx| {
                                let busy = dialog.read(cx).pending;
                                let cancel = cancel.clone();
                                let confirm = confirm.clone();
                                surface
                                    .title("Move selected items")
                                    .overlay_closable(false)
                                    .close_button(!busy)
                                    .on_cancel(move |_, _, cx| {
                                        _ = cancel
                                            .update(cx, |d, cx| d.emit(MoveEvent::Cancel, cx));
                                        false
                                    })
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
                        }
                    }
                    Err(EditError::SessionRejected) => {
                        v.link_input = None;
                        v.screen = Self::identifier(w, cx);
                        v.message = "Session expired. Sign in again.";
                    }
                    Err(EditError::PermissionDenied | EditError::Missing) => {
                        nav.page = Page::Unavailable;
                        nav.share = None;
                        v.message = "Destination unavailable";
                    }
                    Err(_) => {
                        old.reconcile = true;
                        old.set_busy(false, cx);
                        for f in &mut old.failed_moves {
                            f.review = None;
                        }
                        v.message = "Could not read item placement. Reload board or review again.";
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn item_move_event(
        &mut self,
        dialog: Entity<ItemMoveDialog>,
        event: &MoveEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(_, t) = &mut nav.page else {
            return;
        };
        if t.pending || t.item_move.as_ref() != Some(&dialog) {
            return;
        }
        if matches!(event, MoveEvent::Cancel) {
            t.item_move = None;
            t._item_move_subscription = None;
            w.close_dialog(cx);
            cx.notify();
            return;
        }
        let d = dialog.read(cx);
        let Some(group) = d.destination.read(cx).selected_value().cloned() else {
            return;
        };
        let id = d.id;
        let revision = d.revision;
        if d.changed_source {
            t.remember_move(id, group);
            t.item_move = None;
            t._item_move_subscription = None;
            w.close_dialog(cx);
            self.message = "Group changed. Review move.";
            cx.notify();
            return;
        }
        self.save_item_move(id, group, revision, w, cx);
    }

    fn apply_item_move(&mut self, id: Uuid, revision: i64, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        let Page::Items(_, t) = &nav.page else {
            return;
        };
        let Some(f) = t.failed_moves.iter().find(|f| {
            f.id == id
                && f.review
                    .as_ref()
                    .is_some_and(|r| r.revision == Some(revision))
        }) else {
            return;
        };
        let group = f.group.clone();
        self.save_item_move(id, group, revision, w, cx);
    }

    fn save_item_move(
        &mut self,
        id: Uuid,
        group: SharedString,
        revision: i64,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, t) = &mut nav.page else {
            return;
        };
        if t.pending || t.reconcile || !t.editable {
            return;
        }
        let board = *board;
        let visit = self.navigation_request;
        let client = nav.client.clone();
        t.set_busy(true, cx);
        if let Some(d) = &t.item_move {
            d.update(cx, |d, cx| {
                d.pending = true;
                cx.notify();
            });
        }
        self.message = "Moving item…";
        #[cfg(all(windows, feature = "native-smoke"))]
        {
            self.submissions += 1;
        }
        let to = group.to_string();
        let work = cx.background_executor().spawn(async move {
            client
                .move_main_table_item(board, id, &to, revision)
                .map_err(Failure::Save)?;
            client.read_main_table(board).map_err(Failure::Read)
        });
        cx.spawn_in(w, async move |v, cx| {
            let result = work.await;
            _ = v.update_in(cx, |v, w, cx| {
                if v.navigation_request != visit { return; }
                let Screen::Navigation(nav) = &mut v.screen else { return; };
                let Page::Items(current, old) = &mut nav.page else { return; };
                if *current != board || !old.pending { return; }
                if old.item_move.take().is_some() { old._item_move_subscription = None; w.close_dialog(cx); }
                match result {
                    Ok(read) => {
                        let mut t = MainTable::new(read, board, Some(old), w, cx);
                        t.failed_moves.retain(|f| f.id != id);
                        if let Some(focus) = t.archive_triggers.get(&id) { focus.focus(w, cx); }
                        nav.page = Page::Items(board, Box::new(t)); v.message = "";
                    }
                    Err(Failure::Save(EditError::SessionRejected) | Failure::Read(EditError::SessionRejected)) => {
                        v.link_input = None; v.screen = Self::identifier(w, cx); v.message = "Session expired. Sign in again.";
                    }
                    Err(Failure::Read(EditError::PermissionDenied | EditError::Missing)) => {
                        nav.page = Page::Unavailable; nav.share = None; v.message = "Destination unavailable";
                    }
                    Err(failure) => {
                        if matches!(failure, Failure::Save(_)) { old.remember_move(id, group); }
                        else { old.failed_moves.retain(|f| f.id != id); }
                        old.reconcile = true; old.set_busy(false, cx);
                        v.message = match failure {
                            Failure::Save(EditError::PermissionDenied) => "Not saved: permission denied. Checking current access…",
                            Failure::Save(EditError::Conflict) => "Group changed. Review move.",
                            Failure::Save(EditError::InvalidDraft | EditError::Missing) => "Item or destination group unavailable. Review move.",
                            Failure::Read(_) => "Move saved. Reload board to confirm accepted placement.",
                            _ => "Could not confirm move. Review move; it will not be repeated automatically.",
                        };
                        if matches!(failure, Failure::Save(EditError::PermissionDenied)) { v.refresh_item_entry(board, false, w, cx); }
                    }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}
