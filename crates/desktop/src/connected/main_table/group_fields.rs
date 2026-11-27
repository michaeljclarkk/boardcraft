//! The dialog owns raw fields and baselines. Review/adoption never writes.
use super::*;
use boardcraft_mobile_edit_client::GroupFields;
use gpui_kit::component::{
    IndexPath, WindowExt,
    input::InputEvent,
    select::{Select, SelectState},
};

#[derive(Clone, Copy, PartialEq)]
enum State {
    Ready,
    Saving,
    Conflict,
    Reading,
    Reviewed,
    Uncertain,
}

pub(super) struct GroupDialog {
    id: SharedString,
    group_revision: Option<i64>,
    board_revision: i64,
    name: Entity<InputState>,
    colour: Entity<SelectState<Vec<SharedString>>>,
    state: State,
    notice: SharedString,
    review: Option<MainTableRead>,
    trigger: FocusHandle,
    save_focus: FocusHandle,
    _input: Subscription,
}
enum GroupEvent {
    Save,
    Cancel,
    Review,
    Adopt,
}
impl EventEmitter<GroupEvent> for GroupDialog {}
impl GroupDialog {
    fn pending(&self) -> bool {
        matches!(self.state, State::Saving | State::Reading)
    }
    fn valid(&self, cx: &App) -> bool {
        let raw = self.name.read(cx).value();
        !raw.contains('\0') && (1..=160).contains(&raw.trim().encode_utf16().count())
    }
    fn adoptable(&self) -> bool {
        self.review.as_ref().is_some_and(|r| {
            r.is_manageable()
                && if self.group_revision.is_some() {
                    r.groups().iter().any(|g| g.id() == self.id.as_ref())
                } else {
                    !r.groups().iter().any(|g| g.id() == self.id.as_ref())
                }
        })
    }
    fn emit(&self, event: GroupEvent, cx: &mut Context<Self>) {
        if !self.pending()
            && (!matches!(event, GroupEvent::Cancel) || self.state != State::Uncertain)
        {
            cx.emit(event);
        }
    }
}
impl Render for GroupDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = self.pending() || self.state == State::Uncertain;
        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new(match self.group_revision {
                Some(revision) => format!(
                    "Group {} · board revision {} · group revision {revision}",
                    self.id, self.board_revision
                ),
                None => format!("New group · board revision {}", self.board_revision),
            }))
            .child(Label::new("Group name"))
            .child(
                Input::new(&self.name)
                    .id("group-field-name")
                    .aria_label("Group name")
                    .disabled(disabled),
            )
            .child(Label::new("Group colour"))
            .child(
                Select::new(&self.colour)
                    .accessibility_label("Group colour")
                    .disabled(disabled),
            )
            .child(Label::new(self.notice.clone()));
        if !self.valid(cx) {
            body = body.child(Label::new("Enter a group name of 1–160 UTF-16 units."));
        }
        if let Some(read) = &self.review {
            body = body.child(Label::new(format!("Board revision {}", read.revision())));
            if let Some(revision) = self.group_revision {
                body = body.child(Label::new(
                    match read.groups().iter().find(|g| g.id() == self.id.as_ref()) {
                        Some(g) => format!(
                            "{} · {} / {} · revision {} (requested {})",
                            g.id(),
                            g.name(),
                            g.colour(),
                            g.revision(),
                            revision
                        ),
                        None => format!("Group {} is no longer available.", self.id),
                    },
                ));
            } else {
                body = body.child(Label::new(
                    read.groups()
                        .iter()
                        .map(|g| g.name())
                        .collect::<Vec<_>>()
                        .join(", "),
                ));
            }
            body = body.child(
                Button::new("group-field-adopt")
                    .label("Keep my draft and use this revision")
                    .disabled(!self.adoptable() || self.pending())
                    .on_click(cx.listener(|d, _, _, cx| d.emit(GroupEvent::Adopt, cx))),
            );
        }
        if matches!(
            self.state,
            State::Conflict | State::Reviewed | State::Uncertain | State::Reading
        ) {
            body = body.child(
                Button::new("group-field-review")
                    .label(if self.state == State::Uncertain {
                        "Reload board"
                    } else {
                        "Review latest board"
                    })
                    .disabled(self.pending())
                    .on_click(cx.listener(|d, _, _, cx| d.emit(GroupEvent::Review, cx))),
            );
        }
        body.child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("group-field-cancel")
                        .label("Cancel")
                        .disabled(disabled)
                        .on_click(cx.listener(|d, _, _, cx| d.emit(GroupEvent::Cancel, cx))),
                )
                .child(
                    Button::new("group-field-save")
                        .primary()
                        .label("Save group")
                        .track_focus(&self.save_focus)
                        .loading(self.state == State::Saving)
                        .disabled(self.state != State::Ready || !self.valid(cx))
                        .on_click(cx.listener(|d, _, _, cx| d.emit(GroupEvent::Save, cx))),
                ),
        )
    }
}
enum Failure {
    Save(EditError),
    Read(EditError),
}
impl MainTable {
    pub(super) fn new_group_options(&self, cx: &mut Context<Connected>) -> AnyElement {
        let view = cx.weak_entity();
        let manageable = self.manageable;
        Button::new("new-item-options")
            .small()
            .label("New item options")
            .track_focus(&self.group_focus)
            .disabled(self.pending || self.reconcile)
            .dropdown_menu(move |menu, _, _| {
                let view = view.clone();
                menu.item(
                    PopupMenuItem::new("New group of items…")
                        .disabled(!manageable)
                        .on_click(move |_, w, cx| {
                            _ = view.update(cx, |v, cx| v.open_group_fields(None, w, cx));
                        }),
                )
            })
            .into_any_element()
    }
    pub(super) fn group_name(&self, h: &Heading, cx: &mut Context<Connected>) -> AnyElement {
        let id = h.id.clone();
        let geometry = self.drag.clone();
        let geometry_id = id.clone();
        div()
            .on_prepaint(move |bounds, _, _| {
                if let Some(s) = geometry.borrow_mut().section_mut(&geometry_id) {
                    s.set_name(bounds);
                }
            })
            .child(
                Button::new(SharedString::from(format!("group-name-{id}")))
                    .small()
                    .ghost()
                    .label(h.name.clone())
                    .track_focus(&h.name_focus)
                    .disabled(self.pending || self.reconcile || !self.manageable)
                    .on_click(cx.listener(move |v, event: &ClickEvent, w, cx| {
                        if event.click_count() == 2 {
                            v.open_group_fields(Some(&id), w, cx);
                        }
                    })),
            )
            .into_any_element()
    }
}
impl Connected {
    pub(super) fn open_group_fields(
        &mut self,
        id: Option<&str>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(_, table) = &mut nav.page else {
            return;
        };
        if table.pending || table.reconcile || !table.manageable || table.group_dialog.is_some() {
            return;
        }
        let heading = id.and_then(|id| table.headings.iter().find(|h| h.id.as_ref() == id));
        if id.is_some() && heading.is_none() {
            return;
        }
        let name = cx.new(|cx| {
            InputState::new(w, cx).default_value(heading.map_or("New group", |h| h.name.as_ref()))
        });
        let accepted = heading.map_or("#579bfc", |h| h.colour.as_ref());
        let mut colours: Vec<SharedString> = vec!["#579bfc".into(), "#00c875".into()];
        if !colours.iter().any(|c| c.as_ref() == accepted) {
            colours.push(accepted.to_owned().into());
        }
        let ix = colours.iter().position(|c| c.as_ref() == accepted).unwrap();
        let colour = cx.new(|cx| SelectState::new(colours, Some(IndexPath::new(ix)), w, cx));
        let trigger = heading.map_or_else(|| table.group_focus.clone(), |h| h.name_focus.clone());
        let dialog = cx.new(|cx| {
            let input = cx.subscribe(&name, |_, _, _: &InputEvent, cx| cx.notify());
            GroupDialog {
                id: heading
                    .map_or_else(|| format!("g-{}", Uuid::new_v4()).into(), |h| h.id.clone()),
                group_revision: heading.map(|h| h.revision),
                board_revision: table.board_revision,
                name: name.clone(),
                colour,
                trigger: trigger.clone(),
                save_focus: cx.focus_handle().tab_stop(true),
                state: State::Ready,
                notice: "".into(),
                review: None,
                _input: input,
            }
        });
        table._group_subscription = Some(cx.subscribe_in(
            &dialog,
            w,
            |v, d, event: &GroupEvent, w, cx| v.group_fields_event(d.clone(), event, w, cx),
        ));
        table.group_dialog = Some(dialog.clone());
        trigger.focus(w, cx);
        let initial_focus = std::cell::Cell::new(true);
        w.open_dialog(cx, move |surface, w, cx| {
            if initial_focus.replace(false) {
                name.update(cx, |n, cx| {
                    let end = n.value().len();
                    n.set_selected_range(end..end, cx);
                    n.focus(w, cx);
                });
            }
            let d = dialog.read(cx);
            let cancel = dialog.downgrade();
            let save = cancel.clone();
            surface
                .title(if d.group_revision.is_some() {
                    "Edit group"
                } else {
                    "Add group"
                })
                .overlay_closable(false)
                .close_button(!d.pending() && d.state != State::Uncertain)
                .on_cancel(move |_, _, cx| {
                    _ = cancel.update(cx, |d, cx| d.emit(GroupEvent::Cancel, cx));
                    false
                })
                .on_ok(move |_, w, cx| {
                    _ = save.update(cx, |d, cx| {
                        // Selecting a colour with Enter is not a Save.
                        if d.name.focus_handle(cx).is_focused(w) || d.save_focus.is_focused(w) {
                            d.emit(GroupEvent::Save, cx);
                        }
                    });
                    false
                })
                .child(dialog.clone())
        });
        cx.notify();
    }
    fn group_fields_event(
        &mut self,
        dialog: Entity<GroupDialog>,
        event: &GroupEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, table) = &mut nav.page else {
            return;
        };
        if table.pending || table.group_dialog.as_ref() != Some(&dialog) {
            return;
        }
        match event {
            GroupEvent::Cancel => {
                if dialog.read(cx).state == State::Uncertain {
                    return;
                }
                let focus = dialog.read(cx).trigger.clone();
                table.group_dialog = None;
                table._group_subscription = None;
                w.close_dialog(cx);
                focus.focus(w, cx);
                cx.notify();
                return;
            }
            GroupEvent::Adopt => {
                dialog.update(cx, |d, cx| {
                    if d.state != State::Reviewed || !d.adoptable() {
                        return;
                    }
                    let r = d.review.take().unwrap();
                    d.board_revision = r.revision();
                    if d.group_revision.is_some() {
                        d.group_revision = r
                            .groups()
                            .iter()
                            .find(|g| g.id() == d.id.as_ref())
                            .map(|g| g.revision());
                    }
                    d.state = State::Ready;
                    d.notice = "Revision adopted. Save group separately to apply the draft.".into();
                    d.name.update(cx, |n, cx| n.focus(w, cx));
                    cx.notify();
                });
                return;
            }
            GroupEvent::Save
                if dialog.read(cx).state != State::Ready
                    || !dialog.read(cx).valid(cx)
                    || !table.manageable
                    || table.reconcile =>
            {
                return;
            }
            _ => (),
        }
        let board = *board;
        let visit = self.navigation_request;
        let client = nav.client.clone();
        let reviewing = matches!(event, GroupEvent::Review);
        let d = dialog.read(cx);
        let uncertain = d.state == State::Uncertain;
        let command = (
            d.id.to_string(),
            d.group_revision,
            d.board_revision,
            d.name.read(cx).value().to_string(),
            d.colour.read(cx).selected_value().unwrap().to_string(),
        );
        table.set_busy(true, cx);
        dialog.update(cx, |d, cx| {
            d.state = if reviewing {
                State::Reading
            } else {
                State::Saving
            };
            d.notice = if reviewing {
                "Reading current groups…"
            } else {
                "Saving group…"
            }
            .into();
            cx.notify();
        });
        #[cfg(all(windows, feature = "native-smoke"))]
        if !reviewing {
            self.submissions += 1;
        }
        let work = cx.background_executor().spawn(async move {
            if !reviewing {
                let fields = GroupFields::new(command.2, &command.3, &command.4);
                match command.1 {
                    Some(revision) => {
                        client.edit_main_table_group(board, &command.0, revision, &fields)
                    }
                    None => client.create_main_table_group(board, &command.0, &fields),
                }
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
                if *current != board || old.group_dialog.as_ref() != Some(&dialog) { return; }
                old.set_busy(false, cx);
                match result {
                    Ok(read) if reviewing && read.is_manageable() && (!uncertain || {
                        let d = dialog.read(cx);
                        d.group_revision.is_none() && !read.groups().iter().any(|g| g.id() == d.id.as_ref())
                    }) => {
                        // A fresh read proves that an uncertain create has no active
                        // proposed ID. Keep its draft; adoption and Save stay separate.
                        old.reconcile = false;
                        old.set_busy(false, cx);
                        dialog.update(cx, |d, cx| { d.review = Some(read); d.state = State::Reviewed; d.notice = "Review the accepted group fields before adopting this revision.".into(); cx.notify(); });
                    }
                    Ok(read) => {
                        let focus = dialog.read(cx).trigger.clone();
                        let table = MainTable::new(read, board, Some(old), w, cx);
                        w.close_dialog(cx); focus.focus(w, cx);
                        nav.page = Page::Items(board, Box::new(table));
                        v.message = if uncertain { "Board reloaded. Review accepted groups before another change." } else if reviewing { "Group not saved: permission denied." } else { "" };
                    }
                    Err(Failure::Save(EditError::SessionRejected) | Failure::Read(EditError::SessionRejected)) => {
                        w.close_dialog(cx); v.link_input = None; v.screen = Self::identifier(w, cx); v.message = "Session expired. Sign in again.";
                    }
                    Err(Failure::Read(EditError::PermissionDenied | EditError::Missing)) => {
                        w.close_dialog(cx); nav.page = Page::Unavailable; nav.share = None; v.message = "Destination unavailable";
                    }
                    Err(failure) => {
                        let denied = matches!(failure, Failure::Save(EditError::PermissionDenied));
                        dialog.update(cx, |d, cx| {
                            d.review = None;
                            d.state = match &failure {
                                Failure::Save(EditError::InvalidDraft | EditError::HostFault) if !uncertain => State::Ready,
                                Failure::Save(EditError::Conflict | EditError::PermissionDenied) if !uncertain => State::Conflict,
                                Failure::Read(_) if reviewing && !uncertain => State::Conflict,
                                _ => State::Uncertain,
                            };
                            d.notice = if reviewing { match d.state {
                                State::Uncertain => "Could not read latest board. Reload board; this request will not be repeated.",
                                _ => "Could not read latest board. Review latest board to try again.",
                            } } else { match d.state {
                                State::Uncertain => "Group save not confirmed. Reload board; this request will not be repeated.",
                                State::Ready => "Could not save group. Check the draft and save again.",
                                _ => "Board changed. Review latest board before saving again.",
                            } }.into(); cx.notify();
                        });
                        if dialog.read(cx).state == State::Uncertain { old.reconcile = true; old.set_busy(false, cx); }
                        if denied { v.group_fields_event(dialog.clone(), &GroupEvent::Review, w, cx); }
                    }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}
