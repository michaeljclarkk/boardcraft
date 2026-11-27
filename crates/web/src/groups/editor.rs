//! Stock Kit dialog composition. Raw fields and reviewed baselines belong to this editor.
use super::*;
#[derive(Clone)]
pub(super) enum Kind {
    Group(Option<String>),
    Move(Uuid),
    MoveSelection,
    ArchiveSelection,
    DeleteSelection,
    ArchiveRoot(Uuid),
    RestoreRoot(Uuid, i64, bool),
    ArchiveGroup(String),
    RestoreGroup(String),
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
    colour: Entity<InputState>,
    target: Option<String>,
    groups: Vec<(String, String)>,
    pub(super) phase: Phase,
    review: Option<MainTableRead>,
    message: SharedString,
    _events: Subscription,
}
impl Editor {
    fn command(&self, cx: &App) -> Option<TableCommand> {
        if self.phase != Phase::Ready {
            return None;
        }
        Some(match &self.kind {
            Kind::Group(_) => TableCommand::GroupFields {
                id: self.id.clone(),
                revision: self.group_revision,
                board_revision: self.board_revision,
                name: self.name.read(cx).value().to_string(),
                colour: self.colour.read(cx).value().to_string(),
            },
            Kind::Move(id) => TableCommand::Move {
                id: *id,
                group: self.target.clone()?,
                revision: self.items.first()?.1,
            },
            Kind::MoveSelection | Kind::ArchiveSelection | Kind::DeleteSelection => {
                TableCommand::Selection {
                    operation: self.operation,
                    board_revision: self.board_revision,
                    items: self.items.clone(),
                    action: match self.kind {
                        Kind::MoveSelection => SelectionAction::Move(self.target.clone()?),
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
            Kind::RestoreRoot(id, revision, trash) => TableCommand::Root {
                id: *id,
                operation: self.operation,
                board_revision: self.board_revision,
                revision: *revision,
                action: if *trash {
                    RootAction::RestoreDeleted
                } else {
                    RootAction::Restore
                },
            },
            Kind::ArchiveGroup(id) | Kind::RestoreGroup(id) => TableCommand::Group {
                id: id.clone(),
                board_revision: self.board_revision,
                restore: matches!(self.kind, Kind::RestoreGroup(_)),
            },
        })
    }
    fn save(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(command) = self.command(cx) else {
            return;
        };
        self.phase = Phase::Saving;
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
    fn adopt(&mut self, cx: &mut Context<Self>) {
        let Some(read) = self.review.take() else {
            return;
        };
        let manageable = if matches!(self.kind, Kind::Move(_) | Kind::MoveSelection) {
            read.is_editable()
        } else {
            read.is_manageable()
        };
        let mut items = Vec::new();
        for (id, _) in &self.items {
            let Some(row) = read
                .groups()
                .iter()
                .flat_map(|g| g.items())
                .find(|r| r.id == *id)
            else {
                self.message = "An original item is unavailable".into();
                cx.notify();
                return;
            };
            items.push((*id, row.revision));
        }
        if !manageable {
            self.message = "Current access does not allow this change".into();
            cx.notify();
            return;
        }
        if matches!(self.kind, Kind::Group(Some(_))) {
            let Some(group) = read.groups().iter().find(|g| g.id() == self.id) else {
                self.message = "Group unavailable".into();
                cx.notify();
                return;
            };
            self.group_revision = Some(group.revision());
        }
        if matches!(self.kind, Kind::Group(None)) && read.groups().iter().any(|g| g.id() == self.id)
        {
            self.message = "Proposed group already exists. Close and inspect the board.".into();
            cx.notify();
            return;
        }
        if self
            .target
            .as_ref()
            .is_some_and(|id| !read.groups().iter().any(|g| g.id() == id))
        {
            self.message = "Destination unavailable".into();
            cx.notify();
            return;
        }
        self.groups = read
            .groups()
            .iter()
            .map(|g| (g.id().into(), g.name().into()))
            .collect();
        self.items = items;
        self.board_revision = read.revision();
        self.operation = Uuid::new_v4();
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
        let id = group.map_or_else(|| format!("g-{}", Uuid::new_v4()), |g| g.id().into());
        let name = cx.new(|cx| {
            InputState::new(w, cx).default_value(group.map_or("New group", |g| g.name()))
        });
        // Stored user colour, not an application theme value. The host validates the raw draft.
        let colour = cx.new(|cx| {
            InputState::new(w, cx).default_value(group.map_or("#579bfc", |g| g.colour()))
        });
        let ids = match kind {
            Kind::Move(id) | Kind::ArchiveRoot(id) => vec![id],
            Kind::MoveSelection | Kind::ArchiveSelection | Kind::DeleteSelection => {
                self.selection.clone()
            }
            _ => vec![],
        };
        let items = ids
            .into_iter()
            .filter_map(|id| {
                self.read
                    .groups()
                    .iter()
                    .flat_map(|g| g.items())
                    .find(|r| r.id == id)
                    .map(|r| (id, r.revision))
            })
            .collect();
        let board_revision = self
            .inventory
            .as_ref()
            .and_then(|(_, p)| p.first())
            .map_or(self.read.revision(), ArchiveRead::revision);
        let owner = cx.weak_entity();
        let editor = cx.new(|cx| {
            let events = cx.subscribe_in(&name, w, |e: &mut Editor, _, event, w, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    e.save(w, cx);
                }
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
                target: None,
                groups: self
                    .read
                    .groups()
                    .iter()
                    .map(|g| (g.id().into(), g.name().into()))
                    .collect(),
                phase: Phase::Ready,
                review: None,
                message: "".into(),
                _events: events,
            }
        });
        self.editor = Some(editor.clone());
        let weak = cx.weak_entity();
        let cancel = editor.downgrade();
        w.open_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            let cancel = cancel.clone();
            dialog
                .title("Board change")
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
                    true
                })
        });
        name.update(cx, |i, cx| i.focus(w, cx));
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
            let _ = t.update_in(cx, |t, _, cx| {
                t.pending = false;
                match result {
                    Ok(read) => {
                        let _ = editor.update(cx, |e, cx| {
                            e.message = format!(
                                "Board revision {} · {}",
                                read.revision(),
                                read.groups()
                                    .iter()
                                    .map(|g| format!(
                                        "{}: {} / {} revision {}",
                                        g.id(),
                                        g.name(),
                                        g.colour(),
                                        g.revision()
                                    ))
                                    .collect::<Vec<_>>()
                                    .join("; ")
                            )
                            .into();
                            for (id, _) in &e.items {
                                let current = read
                                    .groups()
                                    .iter()
                                    .flat_map(|g| g.items())
                                    .find(|r| r.id == *id);
                                e.message = format!(
                                    "{} · {}",
                                    e.message,
                                    current.map_or_else(
                                        || format!("{id}: unavailable"),
                                        |r| format!("{id}: {} revision {}", r.title, r.revision)
                                    )
                                )
                                .into();
                            }
                            e.review = Some(read);
                            e.phase = Phase::Reviewed;
                            cx.notify();
                        });
                    }
                    Err(e) => {
                        let _ = editor.update(cx, |e, cx| {
                            e.phase = Phase::Conflict;
                            cx.notify();
                        });
                        t.report(e, cx);
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
        body = body.child(status(format!(
            "{} · board revision {}",
            self.id, self.board_revision
        )));
        if matches!(self.kind, Kind::Group(_)) {
            body = body
                .child("Group name")
                .child(
                    Input::new(&self.name)
                        .aria_label("Group name")
                        .disabled(pending),
                )
                .child("Group colour")
                .child(
                    Input::new(&self.colour)
                        .aria_label("Group colour")
                        .disabled(pending),
                );
        }
        if matches!(self.kind, Kind::Move(_) | Kind::MoveSelection) {
            body = body.child("Destination group");
            for (id, name) in &self.groups {
                let target = id.clone();
                body = body.child(
                    Button::new(key(format!("destination-{id}")))
                        .label(name.clone())
                        .accessibility_id(id.clone())
                        .selected(self.target.as_ref() == Some(id))
                        .disabled(pending)
                        .on_click(cx.listener(move |e, _, _, cx| {
                            e.target = Some(target.clone());
                            cx.notify();
                        })),
                );
            }
        }
        for (id, revision) in &self.items {
            body = body.child(format!("{id} · revision {revision}"));
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
                    .on_click(cx.listener(|e, _, _, cx| e.adopt(cx))),
            );
        }
        let label = match self.kind {
            Kind::Group(_) => "Save group",
            Kind::Move(_) | Kind::MoveSelection => "Confirm move",
            Kind::ArchiveSelection | Kind::ArchiveRoot(_) | Kind::ArchiveGroup(_) => "Archive",
            Kind::DeleteSelection => "Delete",
            _ => "Restore",
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
                        .label(label)
                        .disabled(pending || self.command(cx).is_none())
                        .on_click(cx.listener(|e, _, w, cx| e.save(w, cx))),
                ),
        )
    }
}
