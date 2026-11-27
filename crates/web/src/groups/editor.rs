//! Stock Kit dialog composition. Raw fields and reviewed baselines belong to this editor.
use super::*;
use gpui_kit::component::{
    IndexPath,
    select::{Select, SelectItem, SelectState},
};

#[derive(Clone)]
struct Destination {
    id: SharedString,
    name: SharedString,
}
impl SelectItem for Destination {
    type Value = SharedString;
    fn title(&self) -> SharedString {
        self.name.clone()
    }
    fn value(&self) -> &SharedString {
        &self.id
    }
}

#[derive(Clone)]
struct GroupColour(SharedString);
impl GroupColour {
    fn swatch(&self) -> gpui_kit::AnyElement {
        let mut row = div().flex().items_center().gap_2();
        if let Some(rgb) = self
            .0
            .strip_prefix('#')
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        {
            row = row.child(div().size_3().rounded_sm().bg(gpui_kit::rgb(rgb)));
        }
        row.child(self.0.clone()).into_any_element()
    }
}
impl SelectItem for GroupColour {
    type Value = SharedString;
    fn title(&self) -> SharedString {
        self.0.clone()
    }
    fn value(&self) -> &SharedString {
        &self.0
    }
    fn display_title(&self) -> Option<gpui_kit::AnyElement> {
        Some(self.swatch())
    }
    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.swatch()
    }
}
#[derive(Clone)]
pub(super) enum Kind {
    Group(Option<String>),
    Move(Uuid),
    MoveSelection,
    ArchiveSelection,
    DeleteSelection,
    ArchiveRoot(Uuid),
}
#[derive(PartialEq)]
pub(super) enum Phase {
    Ready,
    Saving,
    Conflict,
    Reading,
    Reviewed,
    Uncertain,
}
pub(super) struct Editor {
    owner: WeakEntity<Table>,
    kind: Kind,
    id: String,
    group_revision: Option<i64>,
    board_revision: i64,
    items: Vec<(Uuid, i64)>,
    operation: Uuid,
    name: Entity<InputState>,
    colour: Entity<SelectState<Vec<GroupColour>>>,
    target: Entity<SelectState<Vec<Destination>>>,
    names: Vec<String>,
    confirm_focus: FocusHandle,
    pub(super) phase: Phase,
    review: Option<MainTableRead>,
    pub(super) message: SharedString,
    _events: Subscription,
}
impl Editor {
    fn command(&self, cx: &App) -> Option<TableCommand> {
        if self.phase != Phase::Ready {
            return None;
        }
        if matches!(self.kind, Kind::Group(_)) && self.name.read(cx).value().trim().is_empty() {
            return None;
        }
        Some(match &self.kind {
            Kind::Group(_) => TableCommand::GroupFields {
                id: self.id.clone(),
                revision: self.group_revision,
                board_revision: self.board_revision,
                name: self.name.read(cx).value().to_string(),
                colour: self.colour.read(cx).selected_value()?.to_string(),
            },
            Kind::Move(id) => TableCommand::Move {
                id: *id,
                group: self.target.read(cx).selected_value()?.to_string(),
                revision: self.items.first()?.1,
            },
            Kind::MoveSelection | Kind::ArchiveSelection | Kind::DeleteSelection => {
                TableCommand::Selection {
                    operation: self.operation,
                    board_revision: self.board_revision,
                    items: self.items.clone(),
                    action: match self.kind {
                        Kind::MoveSelection => SelectionAction::Move(
                            self.target.read(cx).selected_value()?.to_string(),
                        ),
                        Kind::ArchiveSelection => SelectionAction::Archive,
                        _ => SelectionAction::Delete,
                    },
                }
            }
            Kind::ArchiveRoot(id) => TableCommand::Root {
                id: *id,
                operation: self.operation,
                board_revision: self.board_revision,
                revision: self.items.first()?.1,
                action: RootAction::Archive,
            },
        })
    }
    fn save(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(command) = self.command(cx) else {
            return;
        };
        self.phase = Phase::Saving;
        self.message = "Saving…".into();
        cx.notify();
        let weak = cx.weak_entity();
        let _ = self
            .owner
            .update(cx, |t, cx| t.submit(command, Some(weak), None, w, cx));
    }
    fn cancel(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.phase, Phase::Saving | Phase::Reading) {
            let _ = self.owner.update(cx, |t, cx| t.close_editor(w, cx));
        }
    }
    fn review_problem(&self, read: &MainTableRead, cx: &App) -> Option<&'static str> {
        let moving = matches!(self.kind, Kind::Move(_) | Kind::MoveSelection);
        if (moving && !read.is_editable()) || (!moving && !read.is_manageable()) {
            return Some("Current access does not allow this change");
        }
        if self.items.iter().any(|(id, _)| {
            !read
                .groups()
                .iter()
                .flat_map(|g| g.items())
                .any(|r| r.id == *id)
        }) {
            return Some("An original item is unavailable");
        }
        if moving
            && !self
                .target
                .read(cx)
                .selected_value()
                .is_some_and(|id| read.groups().iter().any(|g| g.id() == id.as_ref()))
        {
            return Some("Destination unavailable");
        }
        let exists = read.groups().iter().any(|g| g.id() == self.id);
        match self.kind {
            Kind::Group(None) if exists => {
                Some("Proposed group already exists. Close and inspect the board.")
            }
            Kind::Group(Some(_)) if !exists => Some("Group unavailable"),
            _ => None,
        }
    }
    fn adopt(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(read) = self.review.as_ref() else {
            return;
        };
        if self.review_problem(read, cx).is_some() {
            return;
        }
        for ((id, revision), name) in self.items.iter_mut().zip(&mut self.names) {
            if let Some(row) = read
                .groups()
                .iter()
                .flat_map(|g| g.items())
                .find(|r| r.id == *id)
            {
                *revision = row.revision;
                name.clone_from(&row.title);
            }
        }
        if matches!(self.kind, Kind::Group(Some(_))) {
            self.group_revision = read
                .groups()
                .iter()
                .find(|g| g.id() == self.id)
                .map(|g| g.revision());
        }
        if matches!(self.kind, Kind::Move(_) | Kind::MoveSelection) {
            self.target.update(cx, |s, cx| {
                s.set_items(
                    read.groups()
                        .iter()
                        .map(|g| Destination {
                            id: key(g.id()),
                            name: key(g.name()),
                        })
                        .collect(),
                    w,
                    cx,
                )
            });
        }
        self.board_revision = read.revision();
        self.review = None;
        self.message = "Latest revisions adopted. Confirm separately to save.".into();
        self.phase = Phase::Ready;
        let _ = self.owner.update(cx, |t, cx| {
            t.reconcile = false;
            cx.notify();
        });
        cx.notify();
    }
}
impl Table {
    pub(super) fn close_editor(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.editor = None;
        w.close_dialog(cx);
        cx.notify();
    }
    pub(super) fn open_editor(&mut self, kind: Kind, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let is_move = matches!(kind, Kind::Move(_) | Kind::MoveSelection);
        if (is_move && !self.read.is_editable()) || (!is_move && !self.read.is_manageable()) {
            return;
        }
        let group = if let Kind::Group(Some(id)) = &kind {
            self.read.groups().iter().find(|g| g.id() == id)
        } else {
            None
        };
        if matches!(&kind, Kind::Group(Some(_))) && group.is_none() {
            return;
        }
        let id = group.map_or_else(
            || {
                if matches!(kind, Kind::Group(None)) {
                    format!("g-{}", Uuid::new_v4())
                } else {
                    String::new()
                }
            },
            |g| g.id().into(),
        );
        let name = cx.new(|cx| {
            InputState::new(w, cx).default_value(group.map_or("New group", |g| g.name()))
        });
        // Stored user colour, not an application theme value. The host validates the raw draft.
        let accepted = group.map_or("#579bfc", |g| g.colour());
        let mut colours: Vec<SharedString> = vec!["#579bfc".into(), "#00c875".into()];
        let index = colours
            .iter()
            .position(|c| c.as_ref() == accepted)
            .unwrap_or_else(|| {
                colours.push(accepted.to_owned().into());
                colours.len() - 1
            });
        let colour = cx.new(|cx| {
            SelectState::new(
                colours.into_iter().map(GroupColour).collect::<Vec<_>>(),
                Some(IndexPath::new(index)),
                w,
                cx,
            )
        });
        let choices: Vec<_> = self
            .read
            .groups()
            .iter()
            .map(|g| Destination {
                id: key(g.id()),
                name: key(g.name()),
            })
            .collect();
        let first = (!choices.is_empty()).then(|| IndexPath::new(0));
        let target = cx.new(|cx| SelectState::new(choices, first, w, cx));
        let ids = match kind {
            Kind::Move(id) | Kind::ArchiveRoot(id) => vec![id],
            Kind::MoveSelection | Kind::ArchiveSelection | Kind::DeleteSelection => {
                self.selection.clone()
            }
            _ => vec![],
        };
        let rows: Vec<_> = ids
            .iter()
            .copied()
            .filter_map(|id| {
                self.read
                    .groups()
                    .iter()
                    .flat_map(|g| g.items())
                    .find(|r| r.id == id)
            })
            .collect();
        if ids.len() != rows.len() || (rows.is_empty() && !matches!(kind, Kind::Group(_))) {
            return;
        }
        let items = rows.iter().map(|r| (r.id, r.revision)).collect();
        let names = rows.iter().map(|r| r.title.clone()).collect();
        let board_revision = self.read.revision();
        let owner = cx.weak_entity();
        let editor = cx.new(|cx| {
            let events = cx.subscribe_in(&name, w, |e: &mut Editor, _, event, w, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    e.save(w, cx);
                }
                cx.notify();
            });
            Editor {
                owner,
                kind,
                id,
                group_revision: group.map(|g| g.revision()),
                board_revision,
                items,
                operation: Uuid::new_v4(),
                name: name.clone(),
                colour,
                target,
                names,
                confirm_focus: cx.focus_handle().tab_stop(true),
                phase: Phase::Ready,
                review: None,
                message: "".into(),
                _events: events,
            }
        });
        self.editor = Some(editor.clone());
        let weak = cx.weak_entity();
        let cancel = editor.downgrade();
        let initial = std::cell::Cell::new(true);
        w.open_dialog(cx, move |dialog, w, cx| {
            let weak = weak.clone();
            let cancel = cancel.clone();
            let d = editor.read(cx);
            let title = match d.kind {
                Kind::Group(None) => "Add group",
                Kind::Group(Some(_)) => "Edit group",
                Kind::Move(_) | Kind::MoveSelection => "Move selected items",
                Kind::ArchiveRoot(_) | Kind::ArchiveSelection => "Archive selected items?",
                Kind::DeleteSelection => "Delete selected items?",
            };
            let group = matches!(d.kind, Kind::Group(_));
            let pending = matches!(d.phase, Phase::Saving | Phase::Reading);
            let confirm = d.confirm_focus.clone();
            if initial.replace(false) {
                if group {
                    name.update(cx, |n, cx| {
                        let end = n.value().len();
                        n.set_selected_range(end..end, cx);
                        n.focus(w, cx);
                    });
                } else {
                    confirm.focus(w, cx);
                }
            }
            dialog
                .title(title)
                .close_button(!pending)
                .overlay_closable(false)
                .keyboard(false)
                .child(editor.clone())
                .on_close(move |_, _, cx| {
                    let _ = weak.update(cx, |t, cx| {
                        t.editor = None;
                        cx.notify();
                    });
                })
                .on_cancel(move |_, w, cx| {
                    let _ = cancel.update(cx, |e, cx| e.cancel(w, cx));
                    false
                })
        });
        cx.notify();
    }
    fn review_editor(
        &mut self,
        editor: WeakEntity<Editor>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pending {
            return;
        }
        self.pending = true;
        let (binding, board, executor) = (
            self.binding.clone(),
            self.board,
            cx.background_executor().clone(),
        );
        self.request = Some(cx.spawn_in(w, async move |t, cx| {
            let result = session::table(&binding, board, &executor).await;
            let _ = t.update_in(cx, |t, w, cx| {
                t.pending = false;
                match result {
                    Ok(read) => {
                        let _ = editor.update(cx, |e, cx| {
                            let mut summary = vec![format!("Board revision {}", read.revision())];
                            if matches!(e.kind, Kind::Group(_)) {
                                for group in read
                                    .groups()
                                    .iter()
                                    .filter(|g| e.group_revision.is_none() || g.id() == e.id)
                                {
                                    summary.push(format!(
                                        "{}: {} / {} revision {}",
                                        group.id(),
                                        group.name(),
                                        group.colour(),
                                        group.revision()
                                    ));
                                }
                            }
                            for (id, _) in &e.items {
                                let current = read
                                    .groups()
                                    .iter()
                                    .flat_map(|g| g.items())
                                    .find(|r| r.id == *id);
                                summary.push(current.map_or_else(
                                    || format!("{id}: unavailable"),
                                    |r| format!("{id}: {} revision {}", r.title, r.revision),
                                ));
                            }
                            if let Some(problem) = e.review_problem(&read, cx) {
                                summary.push(problem.to_owned());
                            }
                            e.message = summary.join(" · ").into();
                            e.review = Some(read.clone());
                            e.phase = Phase::Reviewed;
                            cx.notify();
                        });
                        t.read = read;
                        t.mount(w, cx);
                        t.reconcile = false;
                    }
                    Err(e) => {
                        if matches!(e, EditError::PermissionDenied | EditError::Missing) {
                            cx.emit(AccessLost);
                        }
                        t.report(&e, cx);
                        let _ = editor.update(cx, |e, cx| {
                            e.message = t.message.into();
                            e.phase = Phase::Conflict;
                            cx.notify();
                        });
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pending = matches!(self.phase, Phase::Saving | Phase::Reading);
        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .capture_key_down(cx.listener(|e, event: &gpui_kit::KeyDownEvent, w, cx| {
                if event.keystroke.key == "escape" {
                    e.cancel(w, cx);
                    cx.stop_propagation();
                }
            }));
        if matches!(self.kind, Kind::Group(_)) {
            body = body
                .child(status(format!(
                    "{} · board revision {}",
                    self.id, self.board_revision
                )))
                .child("Group name")
                .child(
                    Input::new(&self.name)
                        .id("group-name")
                        .aria_label("Group name")
                        .disabled(pending),
                )
                .child("Group colour")
                .child(
                    Select::new(&self.colour)
                        .accessibility_label("Group colour")
                        .disabled(pending),
                );
        }
        if !self.items.is_empty() {
            body = body.child(format!("{} selected", self.items.len()));
            for (name, (id, revision)) in self.names.iter().zip(&self.items).take(6) {
                body = body.child(format!("{name} · {id} · revision {revision}"));
            }
            if self.items.len() > 6 {
                body = body.child(format!("and {} more", self.items.len() - 6));
            }
        }
        if matches!(self.kind, Kind::Move(_) | Kind::MoveSelection) {
            body = body
                .child("Destination board")
                .child(
                    Button::new("current-board")
                        .label("Current board")
                        .disabled(true),
                )
                .child("Destination group")
                .child(
                    Select::new(&self.target)
                        .accessibility_label("Destination group")
                        .disabled(pending),
                );
        } else if !self.items.is_empty() {
            body = body.child(if matches!(self.kind, Kind::DeleteSelection) {
                "Restore these items from board Trash."
            } else {
                "Restore these items from board Archive."
            });
        }
        body = body.child(status(self.message.clone()));
        if matches!(
            self.phase,
            Phase::Conflict | Phase::Uncertain | Phase::Reviewed
        ) {
            body = body.child(Button::new("review").label("Review latest board").on_click(
                cx.listener(|e, _, w, cx| {
                    e.phase = Phase::Reading;
                    let weak = cx.weak_entity();
                    let _ = e.owner.update(cx, |t, cx| t.review_editor(weak, w, cx));
                }),
            ));
        }
        if self.phase == Phase::Reviewed {
            body = body.child(
                Button::new("adopt")
                    .label("Keep my choices and use these revisions")
                    .disabled(
                        !self
                            .review
                            .as_ref()
                            .is_some_and(|r| self.review_problem(r, cx).is_none()),
                    )
                    .on_click(cx.listener(|e, _, w, cx| e.adopt(w, cx))),
            );
        }
        let label = match self.kind {
            Kind::Group(_) => "Save group",
            Kind::Move(_) | Kind::MoveSelection => "Confirm move",
            Kind::ArchiveSelection | Kind::ArchiveRoot(_) => "Archive",
            Kind::DeleteSelection => "Delete",
        };
        body.child(
            div()
                .flex()
                .gap_2()
                .child(
                    Button::new("cancel")
                        .label("Cancel")
                        .disabled(pending)
                        .on_click(cx.listener(|e, _, w, cx| e.cancel(w, cx))),
                )
                .child(
                    Button::new("confirm")
                        .track_focus(&self.confirm_focus)
                        .label(label)
                        .disabled(pending || self.command(cx).is_none())
                        .on_click(cx.listener(|e, _, w, cx| e.save(w, cx))),
                ),
        )
    }
}
