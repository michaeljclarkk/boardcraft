//! Menu and pointer submit one full accepted order. Failure recovery only reads.
use super::*;
use std::time::Instant;

enum Failure {
    Save(EditError),
    Read(EditError),
}
impl MainTable {
    pub(super) fn neighbour_order(&self, id: &str, delta: isize) -> Option<Vec<String>> {
        let ix = self.headings.iter().position(|h| h.id.as_ref() == id)?;
        let to = ix
            .checked_add_signed(delta)
            .filter(|to| *to < self.headings.len())?;
        let mut ids: Vec<_> = self.headings.iter().map(|h| h.id.to_string()).collect();
        ids.swap(ix, to);
        Some(ids)
    }
    pub(super) fn group_drag_events(&self, cx: &mut Context<Connected>) -> impl IntoElement {
        let view = cx.weak_entity();
        // Window capture is necessary for release outside a heading/viewport and for
        // suppressing the release click before a Button, disclosure or row receives it.
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
                            || !t.manageable
                            || t.group_dialog.is_some()
                            || t.move_dialog.is_some()
                            || t.item_move.is_some()
                            || t.archive.is_some()
                            || t.root_command.is_some()
                        {
                            return;
                        }
                        if t.drag.borrow().is_held() {
                            return;
                        }
                        t.drag.borrow_mut().press(e.position);
                        if t.drag.borrow().is_held() {
                            // Button activation normally focuses on release. A held name
                            // needs its own Escape path now, not the previous row Input's.
                            if let Some(h) = t
                                .headings
                                .iter()
                                .find(|h| Some(h.id.as_ref()) == t.drag.borrow().source_id())
                            {
                                h.name_focus.focus(w, cx);
                            }
                            v.tick_group_drag(Instant::now(), w, cx);
                        }
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
                        if !t.drag.borrow().is_held() {
                            return;
                        }
                        if e.pressed_button != Some(MouseButton::Left) {
                            t.drag.borrow_mut().cancel();
                        } else {
                            t.drag.borrow_mut().move_pointer(e.position);
                        }
                        if t.drag.borrow().is_active() {
                            cx.stop_propagation();
                        }
                        cx.notify();
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
                        let order = t.drag.borrow_mut().release(e.position);
                        if t.drag.borrow_mut().consume_click() {
                            cx.stop_propagation();
                        }
                        if let Some(ids) = order {
                            v.reorder_main_table_groups(
                                ids.iter().map(ToString::to_string).collect(),
                                w,
                                cx,
                            );
                        }
                        cx.notify();
                    });
                });
            },
        )
        .absolute()
        .size_full()
    }
}
impl Connected {
    fn tick_group_drag(&mut self, last: Instant, w: &mut Window, cx: &mut Context<Self>) {
        let visit = self.navigation_request;
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        let Page::Items(_, table) = &nav.page else {
            return;
        };
        let generation = table.drag.borrow().generation();
        let view = cx.weak_entity();
        w.on_next_frame(move |w, cx| {
            _ = view.update(cx, |v, cx| {
                if v.navigation_request != visit {
                    return;
                }
                let Screen::Navigation(nav) = &mut v.screen else {
                    return;
                };
                let Page::Items(_, t) = &mut nav.page else {
                    return;
                };
                if !t.drag.borrow().is_held() || t.drag.borrow().generation() != generation {
                    return;
                }
                if !w.is_window_active() {
                    t.drag.borrow_mut().cancel();
                    cx.notify();
                    return;
                }
                let now = Instant::now();
                let velocity = t.drag.borrow().scroll_velocity();
                if velocity != 0. {
                    let offset = v.scroll.offset();
                    let y = (offset.y
                        - px(velocity * now.duration_since(last).as_secs_f32().min(0.05)))
                    .clamp(-v.scroll.max_offset().y, px(0.));
                    v.scroll.set_offset(point(offset.x, y));
                    cx.notify();
                }
                v.tick_group_drag(now, w, cx);
            });
        });
    }
    pub(super) fn move_main_table_group(
        &mut self,
        id: &str,
        delta: isize,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        let Page::Items(_, t) = &nav.page else {
            return;
        };
        let Some(ids) = t.neighbour_order(id, delta) else {
            return;
        };
        self.reorder_main_table_groups(ids, w, cx);
    }
    fn reorder_main_table_groups(
        &mut self,
        ids: Vec<String>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, t) = &mut nav.page else {
            return;
        };
        if t.pending
            || t.reconcile
            || !t.manageable
            || t.group_dialog.is_some()
            || t.move_dialog.is_some()
            || t.item_move.is_some()
        {
            return;
        }
        let board = *board;
        let revision = t.board_revision;
        let client = nav.client.clone();
        let visit = self.navigation_request;
        t.set_busy(true, cx);
        self.message = "Saving group order…";
        #[cfg(all(windows, feature = "native-smoke"))]
        {
            self.submissions += 1;
        }
        let work = cx.background_executor().spawn(async move {
            client
                .reorder_main_table_groups(board, revision, &ids)
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
                    Ok(read) => {
                        let table = MainTable::new(read, board, Some(old), w, cx);
                        nav.page = Page::Items(board, Box::new(table)); v.message = "";
                    }
                    Err(Failure::Save(EditError::SessionRejected) | Failure::Read(EditError::SessionRejected)) => {
                        v.link_input = None; v.screen = Self::identifier(w, cx); v.message = "Session expired. Sign in again.";
                    }
                    Err(Failure::Read(EditError::PermissionDenied | EditError::Missing)) => {
                        nav.page = Page::Unavailable; nav.share = None; v.message = "Destination unavailable";
                    }
                    Err(failure) => {
                        old.reconcile = true; old.set_busy(false, cx);
                        v.message = match failure {
                            Failure::Save(EditError::Conflict) => "Group order changed. Reload board, then choose again.",
                            Failure::Save(EditError::PermissionDenied) => "Not saved: permission denied. Checking current access…",
                            _ => "Group order not confirmed. Reload board; the move will not be repeated.",
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
