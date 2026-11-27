//! Grip-only, group-local reuse of the compared threshold/edge gesture helper.
use super::*;
use boardcraft_mobile_edit_client::ItemReorder;

enum Failure {
    Save(EditError),
    Read(EditError),
}

impl MainTable {
    pub(super) fn item_grip(&self, h: &Heading, id: Uuid, name: &str) -> impl IntoElement {
        let geometry = h.row_drag.clone();
        let key = id.to_string();
        div()
            .on_prepaint(move |bounds, _, _| {
                if let Some(row) = geometry.borrow_mut().section_mut(&key) {
                    row.set_name(bounds);
                }
            })
            .child(
                Button::new(SharedString::from(format!("grip-{id}")))
                    .small()
                    .ghost()
                    .label("⋮⋮")
                    .accessibility_label(format!("Drag {name}"))
                    .disabled(self.pending || self.reconcile || !self.editable),
            )
    }

    pub(super) fn item_drop_marker(
        &self,
        h: &Heading,
        id: Uuid,
        cx: &mut Context<Connected>,
    ) -> AnyElement {
        let drag = h.row_drag.borrow();
        let mut overlay = div().absolute().left_0().top_0().size_full();
        if let Some((target, after)) = drag.edge()
            && target == id.to_string()
        {
            // A physical insertion hairline, not product spacing.
            let line = div()
                .id("item-order-marker")
                .test_support()
                .absolute()
                .left_0()
                .right_0()
                .h(px(1.))
                .bg(cx.theme().primary);
            overlay = overlay.child(if after { line.bottom_0() } else { line.top_0() });
        }
        if drag.moving_id() == Some(id.to_string().as_str()) {
            overlay = overlay.child(
                div()
                    .absolute()
                    .right_0()
                    .bg(cx.theme().popover)
                    .child(Label::new("Moving item")),
            );
        }
        overlay.into_any_element()
    }

    pub(super) fn item_drag_events(&self, cx: &mut Context<Connected>) -> impl IntoElement {
        let view = cx.weak_entity();
        canvas(
            |_, _, _| (),
            move |_, _, w, _| {
                let down = view.clone();
                w.on_mouse_event(move |e: &MouseDownEvent, phase, w, cx| {
                    if phase != DispatchPhase::Capture
                        || e.button != MouseButton::Left
                        || w.last_input_was_touch()
                    {
                        return;
                    }
                    _ = down.update(cx, |v, cx| {
                        let Screen::Navigation(nav) = &mut v.screen else {
                            return;
                        };
                        let Page::Items(_, t) = &mut nav.page else {
                            return;
                        };
                        if t.pending
                            || t.reconcile
                            || !t.editable
                            || t.archive.is_some()
                            || t.root_command.is_some()
                            || t.group_dialog.is_some()
                            || t.move_dialog.is_some()
                            || t.item_move.is_some()
                            || t.drag.borrow().is_held()
                        {
                            return;
                        }
                        for h in &t.headings {
                            if h.collapsed {
                                continue;
                            }
                            h.row_drag.borrow_mut().press(e.position);
                            let source = h.row_drag.borrow().source_id().map(str::to_owned);
                            if let Some(source) = source
                                && let Some(row) = h.rows.iter().find(|r| {
                                    r.read(cx).edit.identity().1.as_uuid().to_string() == source
                                })
                            {
                                let focus = row.read(cx).display_focus.clone();
                                focus.focus(w, cx);
                            }
                        }
                        v.tick_item_drag(w, cx);
                    });
                });
                let moving = view.clone();
                w.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    _ = moving.update(cx, |v, cx| {
                        let Screen::Navigation(nav) = &mut v.screen else {
                            return;
                        };
                        let Page::Items(_, t) = &mut nav.page else {
                            return;
                        };
                        for h in &t.headings {
                            let mut drag = h.row_drag.borrow_mut();
                            if !drag.is_held() {
                                continue;
                            }
                            if e.pressed_button == Some(MouseButton::Left) {
                                drag.move_pointer(e.position);
                            } else {
                                drag.cancel();
                            }
                            if drag.is_active() {
                                cx.stop_propagation();
                            }
                            cx.notify();
                        }
                    });
                });
                let up = view.clone();
                w.on_mouse_event(move |e: &MouseUpEvent, phase, w, cx| {
                    if phase != DispatchPhase::Capture || e.button != MouseButton::Left {
                        return;
                    }
                    _ = up.update(cx, |v, cx| {
                        let Screen::Navigation(nav) = &mut v.screen else {
                            return;
                        };
                        let Page::Items(_, t) = &mut nav.page else {
                            return;
                        };
                        let mut command = None;
                        for h in &t.headings {
                            let mut drag = h.row_drag.borrow_mut();
                            let source = drag.source_id().and_then(|id| id.parse::<Uuid>().ok());
                            // Release updates the pointer but must not activate a short click.
                            if drag.is_held() {
                                let edge = drag.release_edge(e.position);
                                if let (Some(source), Some((anchor, after))) = (source, edge)
                                    && let Ok(anchor) = anchor.parse::<Uuid>()
                                {
                                    command = Some((h.id.to_string(), source, anchor, after));
                                }
                                cx.notify();
                            }
                            if drag.consume_click() {
                                cx.stop_propagation();
                            }
                        }
                        if let Some((group, item, anchor, after)) = command {
                            v.reorder_main_table_item(group, item, anchor, after, w, cx);
                        }
                    });
                });
            },
        )
        .absolute()
        .size_full()
    }
}

impl Connected {
    fn tick_item_drag(&self, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        let Page::Items(_, t) = &nav.page else {
            return;
        };
        let Some(drag) = t
            .headings
            .iter()
            .map(|h| h.row_drag.clone())
            .find(|d| d.borrow().is_held())
        else {
            return;
        };
        let generation = drag.borrow().generation();
        let visit = self.navigation_request;
        let view = cx.weak_entity();
        w.on_next_frame(move |w, cx| {
            _ = view.update(cx, |v, cx| {
                if v.navigation_request != visit
                    || !drag.borrow().is_held()
                    || drag.borrow().generation() != generation
                {
                    return;
                }
                if !w.is_window_active() {
                    drag.borrow_mut().cancel();
                    cx.notify();
                    return;
                }
                v.tick_item_drag(w, cx);
            });
        });
    }

    fn reorder_main_table_item(
        &mut self,
        group: String,
        item: Uuid,
        anchor: Uuid,
        after: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, t) = &mut nav.page else {
            return;
        };
        if t.pending || t.reconcile || !t.editable || t.item_move.is_some() {
            return;
        }
        let Some(h) = t.headings.iter().find(|h| h.id.as_ref() == group) else {
            return;
        };
        let Some(row) = h
            .rows
            .iter()
            .find(|r| r.read(cx).edit.identity().1.as_uuid() == item)
        else {
            return;
        };
        if !h
            .rows
            .iter()
            .any(|r| r.read(cx).edit.identity().1.as_uuid() == anchor)
        {
            return;
        }
        let command = ItemReorder::new(
            Uuid::new_v4(),
            group,
            anchor,
            after,
            t.board_revision,
            row.read(cx).edit.revision(),
        );
        let board = *board;
        let visit = self.navigation_request;
        let client = nav.client.clone();
        t.set_busy(true, cx);
        self.message = "Saving item order…";
        #[cfg(all(windows, feature = "native-smoke"))]
        {
            self.submissions += 1;
        }
        let work = cx.background_executor().spawn(async move {
            client
                .reorder_main_table_item(board, item, &command)
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
                match result {
                    Ok(read) => { let t = MainTable::new(read, board, Some(old), w, cx); nav.page = Page::Items(board, Box::new(t)); v.message = ""; }
                    Err(Failure::Save(EditError::SessionRejected) | Failure::Read(EditError::SessionRejected)) => {
                        v.link_input = None; v.screen = Self::identifier(w, cx); v.message = "Session expired. Sign in again.";
                    }
                    Err(Failure::Read(EditError::PermissionDenied | EditError::Missing)) => {
                        nav.page = Page::Unavailable; nav.share = None; v.message = "Destination unavailable";
                    }
                    Err(failure) => {
                        old.reconcile = true; old.set_busy(false, cx);
                        v.message = match failure {
                            Failure::Save(EditError::Conflict) => "Item order changed. Reload board, then drag again.",
                            Failure::Save(EditError::PermissionDenied) => "Not saved: permission denied. Checking current access…",
                            _ => "Item order not confirmed. Reload board; the drop will not be repeated.",
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
