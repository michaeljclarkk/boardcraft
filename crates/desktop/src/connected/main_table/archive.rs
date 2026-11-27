//! Visit-local recovery UI. Only authorized reads replace accepted snapshots.
use super::*;
use boardcraft_mobile_edit_client::ArchiveRead;
use gpui_kit::component::WindowExt;

pub(super) struct Undo {
    id: SharedString,
    pub(super) label: SharedString,
}
#[derive(Clone)]
pub(super) struct RootCommand {
    id: Uuid,
    name: SharedString,
    revision: i64,
    operation: Uuid,
    confirm_focus: FocusHandle,
}
struct Root {
    id: Uuid,
    label: SharedString,
    revision: i64,
    restorable: bool,
    reason: &'static str,
}
struct Group {
    id: SharedString,
    name: SharedString,
}
pub(super) struct Archive {
    revision: i64,
    manageable: bool,
    total: i64,
    more: bool,
    next: Option<i64>,
    roots: Vec<Root>,
    groups: Vec<Group>,
}
impl From<ArchiveRead> for Archive {
    fn from(r: ArchiveRead) -> Self {
        Self {
            revision: r.revision(),
            manageable: r.is_manageable(),
            total: r.total(),
            more: r.has_more(),
            next: r.next_group_offset(),
            roots: r
                .items()
                .iter()
                .map(|i| Root {
                    id: i.id(),
                    label: format!("{} · {}", i.title(), i.group_name()).into(),
                    revision: i.revision(),
                    restorable: i.is_restorable(),
                    reason: if !i.is_group_active() {
                        "Restore its group first."
                    } else if !r.is_manageable() {
                        "Only an Owner can restore items."
                    } else {
                        ""
                    },
                })
                .collect(),
            groups: r
                .groups()
                .iter()
                .map(|g| Group {
                    id: g.id().to_owned().into(),
                    name: g.name().to_owned().into(),
                })
                .collect(),
        }
    }
}
impl MainTable {
    pub(super) fn root_archive_menu(
        &self,
        id: Uuid,
        cx: &mut Context<Connected>,
    ) -> impl IntoElement {
        let v = cx.weak_entity();
        let allowed = self.manageable;
        let movable = self.editable;
        Button::new(SharedString::from(format!("item-menu-{id}")))
            .track_focus(self.archive_triggers.get(&id).expect("active root trigger"))
            .small()
            .ghost()
            .label("More options")
            .disabled(self.pending || self.reconcile)
            .dropdown_menu(move |menu, _, _| {
                let moving = v.clone();
                let menu = menu.item(PopupMenuItem::new("Move to…").disabled(!movable).on_click(
                    move |_, w, cx| {
                        _ = moving.update(cx, |v, cx| v.open_item_move(id, w, cx));
                    },
                ));
                let v = v.clone();
                menu.item(PopupMenuItem::new("Archive…").disabled(!allowed).on_click(
                    move |_, w, cx| {
                        _ = v.update(cx, |v, cx| v.confirm_root_archive(id, w, cx));
                    },
                ))
            })
    }
    pub(super) fn render_archive(&self, a: &Archive, cx: &mut Context<Connected>) -> Div {
        let disabled = self.pending || self.reconcile;
        let mut body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new("Archive"))
            .child(
                Button::new("archive-reload")
                    .small()
                    .label("Reload recovery list")
                    .track_focus(&self.archive_focus)
                    .disabled(self.pending)
                    .on_click(cx.listener(|v, _, w, cx| v.read_archive(None, w, cx))),
            )
            .child(
                Button::new("archive-close")
                    .small()
                    .label("Close Archive")
                    .disabled(self.pending)
                    .on_click(cx.listener(|v, _, w, cx| {
                        if let Screen::Navigation(nav) = &mut v.screen
                            && let Page::Items(_, t) = &mut nav.page
                        {
                            t.archive = None;
                            t.archive_focus.focus(w, cx);
                            cx.notify();
                        }
                    })),
            )
            .child(Label::new("Groups"));
        if !a.manageable {
            body = body.child(Label::new("Only an Owner can archive or restore content."));
        }
        if a.groups.is_empty() {
            body = body.child(Label::new("No archived groups on this page"));
        }
        for g in &a.groups {
            let id = g.id.clone();
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Label::new(g.name.clone()))
                    .child(
                        Button::new(SharedString::from(format!("restore-group-{id}")))
                            .small()
                            .label("Restore group")
                            .disabled(disabled || !a.manageable)
                            .on_click(cx.listener(move |v, _, w, cx| {
                                v.change_archive_group(&id, true, w, cx)
                            })),
                    ),
            );
        }
        if let Some(offset) = a.next {
            let revision = a.revision;
            body = body.child(
                Button::new("archive-next-groups")
                    .small()
                    .label("More archived groups")
                    .disabled(self.pending || self.reconcile)
                    .on_click(cx.listener(move |v, _, w, cx| {
                        v.read_archive(Some((revision, offset)), w, cx)
                    })),
            );
        }
        body = body.child(Label::new(format!("{} archived items", a.total)));
        if a.total == 0 {
            body = body.child(Label::new("No archived items"));
        }
        if a.more {
            body = body.child(Label::new("Showing the first 100 archived items."));
        }
        for r in &a.roots {
            let id = r.id;
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Label::new(r.label.clone()))
                    .child(Label::new(r.reason))
                    .child(
                        Button::new(SharedString::from(format!("restore-item-{id}")))
                            .small()
                            .label("Restore item")
                            .disabled(disabled || !r.restorable)
                            .on_click(
                                cx.listener(move |v, _, w, cx| v.restore_archive_root(id, w, cx)),
                            ),
                    ),
            );
        }
        body
    }
}

enum Command {
    Root {
        id: Uuid,
        revision: i64,
        operation: Uuid,
        restore: bool,
    },
    Group {
        id: SharedString,
        restore: bool,
    },
}
enum Failure {
    Save(EditError),
    Read(EditError),
}

fn snapshot(
    client: &EditClient,
    board: Uuid,
    cursor: Option<(i64, i64)>,
) -> Result<(MainTableRead, ArchiveRead), EditError> {
    let table = client.read_main_table(board)?;
    let archive = client.read_main_table_archive(board, cursor)?;
    if table.revision() != archive.revision() {
        return Err(EditError::Conflict);
    }
    Ok((table, archive))
}

impl Connected {
    fn confirm_root_archive(&mut self, id: Uuid, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(_, t) = &mut nav.page else {
            return;
        };
        if t.pending || t.reconcile || !t.manageable || t.root_command.is_some() {
            return;
        }
        let Some(row) = t
            .headings
            .iter()
            .flat_map(|h| &h.rows)
            .find(|r| r.read(cx).edit.identity().1.as_uuid() == id)
        else {
            return;
        };
        let row = row.read(cx);
        t.root_command = Some(RootCommand {
            id,
            name: row.edit.accepted().to_owned().into(),
            revision: row.edit.revision(),
            operation: Uuid::new_v4(),
            confirm_focus: cx.focus_handle().tab_stop(true),
        });
        t.archive_triggers
            .get(&id)
            .expect("active root trigger")
            .focus(w, cx);
        let weak = cx.weak_entity();
        w.open_dialog(cx, move |d, _, cx| {
            let Some(v) = weak.upgrade() else {
                return d;
            };
            let Screen::Navigation(nav) = &v.read(cx).screen else {
                return d;
            };
            let Page::Items(_, t) = &nav.page else {
                return d;
            };
            let Some(command) = &t.root_command else {
                return d;
            };
            let busy = t.pending;
            let cancel = weak.clone();
            let ok = weak.clone();
            let cancel_button = weak.clone();
            let ok_button = weak.clone();
            d.title("Archive selected items?")
                .overlay_closable(false)
                .close_button(!busy)
                .on_cancel(move |_, w, cx| {
                    _ = cancel.update(cx, |v, cx| v.cancel_root_archive(w, cx));
                    false
                })
                .on_ok(move |_, w, cx| {
                    _ = ok.update(cx, |v, cx| {
                        if let Screen::Navigation(nav) = &v.screen
                            && let Page::Items(_, t) = &nav.page
                            && t.root_command
                                .as_ref()
                                .is_some_and(|r| r.confirm_focus.is_focused(w))
                        {
                            v.submit_root_archive(w, cx);
                        }
                    });
                    false
                })
                .child(Label::new(command.name.clone()))
                .child(Label::new("Restore this item later from board Archive."))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("archive-cancel")
                                .label("Cancel")
                                .disabled(busy)
                                .on_click(move |_, w, cx| {
                                    _ = cancel_button
                                        .update(cx, |v, cx| v.cancel_root_archive(w, cx));
                                }),
                        )
                        .child(
                            Button::new("archive-confirm")
                                .track_focus(&command.confirm_focus)
                                .primary()
                                .label("Archive")
                                .loading(busy)
                                .disabled(busy)
                                .on_click(move |_, w, cx| {
                                    _ = ok_button.update(cx, |v, cx| v.submit_root_archive(w, cx));
                                }),
                        ),
                )
        });
        cx.notify();
    }
    fn cancel_root_archive(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Screen::Navigation(nav) = &mut self.screen
            && let Page::Items(_, t) = &mut nav.page
            && !t.pending
        {
            let focus = t
                .root_command
                .take()
                .and_then(|r| t.archive_triggers.get(&r.id).cloned());
            w.close_dialog(cx);
            if let Some(focus) = focus {
                focus.focus(w, cx);
            }
            cx.notify();
        }
    }
    fn submit_root_archive(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        let Page::Items(_, t) = &nav.page else {
            return;
        };
        let Some(r) = &t.root_command else {
            return;
        };
        self.write_archive(
            Command::Root {
                id: r.id,
                revision: r.revision,
                operation: r.operation,
                restore: false,
            },
            w,
            cx,
        );
    }
    fn restore_archive_root(&mut self, id: Uuid, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        let Page::Items(_, t) = &nav.page else {
            return;
        };
        let Some(r) = t
            .archive
            .as_ref()
            .and_then(|a| a.roots.iter().find(|r| r.id == id && r.restorable))
        else {
            return;
        };
        self.write_archive(
            Command::Root {
                id,
                revision: r.revision,
                operation: Uuid::new_v4(),
                restore: true,
            },
            w,
            cx,
        );
    }
    pub(super) fn change_archive_group(
        &mut self,
        id: &str,
        restore: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        let Page::Items(_, t) = &nav.page else {
            return;
        };
        let available = if restore {
            t.archive
                .as_ref()
                .is_some_and(|a| a.groups.iter().any(|g| g.id.as_ref() == id))
                || t.undo.as_ref().is_some_and(|u| u.id.as_ref() == id)
        } else {
            t.headings.iter().any(|h| h.id.as_ref() == id)
        };
        if available {
            self.write_archive(
                Command::Group {
                    id: id.to_owned().into(),
                    restore,
                },
                w,
                cx,
            );
        }
    }
    pub(super) fn undo_group_archive(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Screen::Navigation(nav) = &self.screen
            && let Page::Items(_, t) = &nav.page
            && let Some(u) = &t.undo
        {
            self.change_archive_group(&u.id.clone(), true, w, cx);
        }
    }
    pub(super) fn read_archive(
        &mut self,
        cursor: Option<(i64, i64)>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.archive_request(None, cursor, w, cx);
    }
    fn write_archive(&mut self, command: Command, w: &mut Window, cx: &mut Context<Self>) {
        self.archive_request(Some(command), None, w, cx);
    }
    fn archive_request(
        &mut self,
        command: Option<Command>,
        cursor: Option<(i64, i64)>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, old) = &mut nav.page else {
            return;
        };
        let writing = command.is_some();
        if old.pending || (writing && (old.reconcile || !old.manageable)) {
            return;
        }
        let board = *board;
        let visit = self.navigation_request;
        let baseline = old
            .archive
            .as_ref()
            .map_or(old.board_revision, |a| a.revision);
        let show_archive = !writing || old.archive.is_some();
        let undo = match &command {
            Some(Command::Group { id, restore: false }) => {
                old.headings.iter().find(|h| h.id == *id).map(|h| Undo {
                    id: id.clone(),
                    label: format!("{} was removed.", h.name).into(),
                })
            }
            _ => None,
        };
        let client = nav.client.clone();
        old.set_busy(true, cx);
        self.message = if writing {
            "Saving archive change…"
        } else {
            "Reading board Archive…"
        };
        #[cfg(all(windows, feature = "native-smoke"))]
        if writing {
            self.submissions += 1;
        }
        let work = cx.background_executor().spawn(async move {
            match command {
                Some(Command::Root {
                    id,
                    revision,
                    operation,
                    restore,
                }) => client
                    .archive_main_table_item(board, id, operation, baseline, revision, restore)
                    .map_err(Failure::Save)?,
                Some(Command::Group { id, restore }) => client
                    .archive_main_table_group(board, &id, baseline, restore)
                    .map_err(Failure::Save)?,
                None => (),
            }
            snapshot(&client, board, cursor).map_err(Failure::Read)
        });
        cx.spawn_in(w, async move |v, cx| {
            let result = work.await;
            _ = v.update_in(cx, |v, w, cx| {
                if visit != v.navigation_request { return; }
                let Screen::Navigation(nav) = &mut v.screen else { return; };
                let Page::Items(current, old) = &mut nav.page else { return; };
                if *current != board || !old.pending { return; }
                if old.root_command.take().is_some() { w.close_dialog(cx); }
                match result {
                    Ok((read, archive)) => {
                        let mut table = MainTable::new(read, board, Some(old), w, cx);
                        if show_archive { table.archive = Some(archive.into()); table.archive_focus.focus(w, cx); }
                        else { table.focus_first(w, cx); }
                        table.undo = undo;
                        nav.page = Page::Items(board, Box::new(table));
                        v.message = if writing { "" } else { "Archive reloaded. No change was submitted." };
                    }
                    Err(Failure::Save(EditError::SessionRejected) | Failure::Read(EditError::SessionRejected)) => {
                        v.link_input = None; v.screen = Self::identifier(w, cx); v.message = "Session expired. Sign in again.";
                    }
                    Err(Failure::Read(EditError::PermissionDenied | EditError::Missing)) => {
                        nav.page = Page::Unavailable; nav.share = None; v.message = "Destination unavailable";
                    }
                    Err(failure) => {
                        old.reconcile = true; old.undo = None; old.set_busy(false, cx);
                        old.archive_focus.focus(w, cx);
                        let denied = matches!(failure, Failure::Save(EditError::PermissionDenied));
                        v.message = if denied { "Archive not saved: permission denied. Checking current access…" }
                            else if matches!(failure, Failure::Save(EditError::Conflict | EditError::Missing | EditError::InvalidDraft)) { "Content changed or is unavailable. Reload Archive before choosing again." }
                            else { "Archive change not confirmed. Reload Archive; this request will not be repeated." };
                        if denied { v.read_archive(None, w, cx); }
                    }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}
