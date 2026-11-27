//! A selected-board visit: drafts and gestures here, accepted records at the host.
use crate::{
    board_view::{NameRow, Rename},
    session,
};
use boardcraft_mobile_edit_client::{
    ArchiveRead, EditError, ItemReorder, MainTableRead, RootAction, SelectionAction, TableCommand,
};
use boardcraft_ui::HeadingDrag;
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Render, Role, SharedString, StatefulInteractiveElement, Styled, Subscription,
    Task, WeakEntity, Window,
    base::{Disableable, Selectable},
    component::{
        ActiveTheme, WindowExt,
        button::Button,
        checkbox::Checkbox,
        input::{Input, InputEvent, InputState},
        menu::{DropdownMenu, PopupMenuItem},
    },
    div,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use uuid::Uuid;
mod editor;
mod gestures;
use editor::{Editor, Kind, Phase};

pub(super) struct AccessLost;
impl EventEmitter<AccessLost> for Table {}
struct Entry {
    input: Entity<InputState>,
    proposed: Uuid,
    _events: Subscription,
}
pub(super) struct Table {
    binding: String,
    board: Uuid,
    read: MainTableRead,
    rows: HashMap<Uuid, Entity<NameRow>>,
    entries: HashMap<String, Entry>,
    selection: Vec<Uuid>,
    anchor: Option<Uuid>,
    folds: HashMap<String, bool>,
    pending: bool,
    reconcile: bool,
    message: &'static str,
    editor: Option<Entity<Editor>>,
    inventory: Option<(bool, Vec<ArchiveRead>)>,
    drag: Rc<RefCell<HeadingDrag>>,
    row_drags: HashMap<String, Rc<RefCell<HeadingDrag>>>,
    focus: FocusHandle,
    request: Option<Task<()>>,
    saves: Vec<Task<()>>,
    subscriptions: Vec<Subscription>,
}
impl Table {
    pub(super) fn new(
        binding: String,
        board: Uuid,
        read: MainTableRead,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut table = Self {
            binding,
            board,
            read,
            rows: HashMap::new(),
            entries: HashMap::new(),
            selection: Vec::new(),
            anchor: None,
            folds: HashMap::new(),
            pending: false,
            reconcile: false,
            message: "",
            editor: None,
            inventory: None,
            drag: Rc::new(RefCell::new(HeadingDrag::default())),
            row_drags: HashMap::new(),
            focus: cx.focus_handle(),
            request: None,
            saves: Vec::new(),
            subscriptions: Vec::new(),
        };
        table.mount(w, cx);
        table
    }
    fn order(&self) -> Vec<Uuid> {
        self.read
            .groups()
            .iter()
            .flat_map(|g| g.items().iter().map(|r| r.id))
            .collect()
    }
    fn local_folds(&mut self, ids: &[String], collapsed: bool) {
        for id in ids {
            self.folds.insert(id.clone(), collapsed);
        }
    }
    fn folded(&self, id: &str, accepted: bool) -> bool {
        self.folds.get(id).copied().unwrap_or(accepted)
    }
    fn mount(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let available = self.order();
        self.selection.retain(|id| available.contains(id));
        if self.anchor.is_some_and(|id| !available.contains(&id)) {
            self.anchor = None;
        }
        self.rows.retain(|id, _| available.contains(id));
        self.entries
            .retain(|id, _| self.read.groups().iter().any(|g| g.id() == id));
        self.subscriptions.clear();
        self.row_drags.clear();
        self.drag = Rc::new(RefCell::new(HeadingDrag::new(
            self.read.groups().iter().map(|g| g.id().to_owned().into()),
        )));
        for group in self.read.groups() {
            self.row_drags.insert(
                group.id().into(),
                Rc::new(RefCell::new(HeadingDrag::new(
                    group.items().iter().map(|r| r.id.to_string().into()),
                ))),
            );
            if !self.entries.contains_key(group.id()) {
                let input = cx.new(|cx| InputState::new(w, cx));
                let id = group.id().to_owned();
                let events = cx.subscribe_in(&input, w, move |t, _, event, w, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        t.create(&id, w, cx);
                    }
                });
                self.entries.insert(
                    group.id().into(),
                    Entry {
                        input,
                        proposed: Uuid::new_v4(),
                        _events: events,
                    },
                );
            }
            for saved in group.items() {
                let row = self
                    .rows
                    .entry(saved.id)
                    .or_insert_with(|| {
                        cx.new(|cx| NameRow::new(saved.clone(), self.read.is_editable(), w, cx))
                    })
                    .clone();
                row.update(cx, |r, cx| r.sync(saved, self.read.is_editable(), cx));
                self.subscriptions.push(cx.subscribe_in(
                    &row,
                    w,
                    |t, row, intent: &Rename, w, cx| t.rename(row.clone(), intent, w, cx),
                ));
            }
        }
        cx.notify();
    }
    fn busy(&self) -> bool {
        self.pending || self.reconcile || self.editor.is_some()
    }
    fn report(&mut self, error: EditError, cx: &mut Context<Self>) {
        self.message = match error {
            EditError::Conflict => "Board changed elsewhere. Reload before choosing again.",
            EditError::InvalidDraft => "Change rejected. Correct the draft.",
            EditError::PermissionDenied => "Change denied. Reload to check current access.",
            EditError::Missing => "Record unavailable. Reload the board.",
            EditError::SessionRejected => "Session ended",
            _ => "Result unconfirmed. Reload before choosing again.",
        };
        if error == EditError::SessionRejected {
            cx.emit(AccessLost);
        }
        cx.notify();
    }
    fn reload(&mut self, w: &mut Window, cx: &mut Context<Self>) {
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
                        t.read = read;
                        t.reconcile = false;
                        t.message = "";
                        t.mount(w, cx);
                    }
                    Err(e) => t.report(e, cx),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn create(&mut self, group: &str, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || !self.read.is_editable() {
            return;
        }
        let Some(entry) = self.entries.get(group) else {
            return;
        };
        let command = TableCommand::Create {
            id: entry.proposed,
            group: group.into(),
            title: entry.input.read(cx).value().to_string(),
        };
        self.submit(command, None, Some(group.into()), w, cx);
    }
    fn submit(
        &mut self,
        command: TableCommand,
        editor: Option<WeakEntity<Editor>>,
        entry: Option<String>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pending || self.reconcile {
            return;
        }
        self.pending = true;
        self.message = "";
        let (binding, board, executor) = (
            self.binding.clone(),
            self.board,
            cx.background_executor().clone(),
        );
        let inventory = self.inventory.as_ref().map(|(trash, _)| *trash);
        self.request = Some(cx.spawn_in(w, async move |t, cx| {
            let saved = session::command(&binding, board, &command, &executor).await;
            let read = if saved.is_ok() {
                Some(session::table(&binding, board, &executor).await)
            } else {
                None
            };
            let recovery = if saved.is_ok() {
                if let Some(trash) = inventory {
                    Some(session::recovery(&binding, board, trash, None, &executor).await)
                } else {
                    None
                }
            } else {
                None
            };
            let _ = t.update_in(cx, |t, w, cx| {
                t.pending = false;
                match saved {
                    Ok(()) => {
                        if let Some(group) = entry.as_ref()
                            && let Some(e) = t.entries.get_mut(group)
                        {
                            e.input.update(cx, |i, cx| i.set_value("", w, cx));
                            e.proposed = Uuid::new_v4();
                        }
                        if matches!(command, TableCommand::Selection { .. }) {
                            t.selection.clear();
                        }
                        if let Some(Ok(read)) = read {
                            t.read = read;
                            t.mount(w, cx);
                            t.reconcile = false;
                        } else {
                            t.reconcile = true;
                            t.message = "Saved, but the board could not be read. Reload board.";
                        }
                        if let Some(result) = recovery {
                            match result {
                                Ok(r) => t.inventory = Some((inventory.unwrap_or(false), vec![r])),
                                Err(e) => {
                                    t.reconcile = true;
                                    t.report(e, cx);
                                }
                            }
                        }
                        if editor.is_some() {
                            t.close_editor(w, cx);
                        }
                    }
                    Err(error) => {
                        let uncertain = matches!(
                            error,
                            EditError::Transport
                                | EditError::HostFault
                                | EditError::AccessUnavailable
                        );
                        t.reconcile = uncertain
                            || matches!(error, EditError::PermissionDenied | EditError::Missing);
                        if let Some(e) = editor {
                            let _ = e.update(cx, |e, cx| {
                                e.phase = if error == EditError::InvalidDraft {
                                    Phase::Ready
                                } else if uncertain {
                                    Phase::Uncertain
                                } else {
                                    Phase::Conflict
                                };
                                cx.notify();
                            });
                        }
                        // A failed non-editor command has no replay action. A fresh read precedes any new intent.
                        if entry.is_none() && t.editor.is_none() {
                            t.reconcile = true;
                        }
                        t.report(error, cx);
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn fold(&mut self, ids: Vec<String>, collapsed: bool, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        if !self.read.is_editable() {
            // Viewer overrides are visit-local; they never masquerade as saved presentation.
            self.local_folds(&ids, collapsed);
            cx.notify();
            return;
        }
        self.submit(
            TableCommand::Fold {
                view_revision: self.read.view_revision(),
                ids,
                collapsed,
            },
            None,
            None,
            w,
            cx,
        );
    }
    fn select(
        &mut self,
        ids: Vec<Uuid>,
        checked: bool,
        anchor: Option<Uuid>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() {
            return;
        }
        // Range/group actions resolve current ordered membership before changing checks.
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
                        let order: Vec<_> = read
                            .groups()
                            .iter()
                            .flat_map(|g| g.items().iter().map(|r| r.id))
                            .collect();
                        if ids.iter().all(|id| order.contains(id)) {
                            t.selection.retain(|id| !ids.contains(id));
                            if checked {
                                t.selection.extend(ids);
                            }
                            if let Some(anchor) = anchor {
                                t.anchor = Some(anchor);
                            }
                            t.read = read;
                            t.message = "";
                            t.mount(w, cx);
                        } else {
                            t.message = "Selection changed. Reload board.";
                        }
                    }
                    Err(e) => t.report(e, cx),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn select_row(
        &mut self,
        id: Uuid,
        checked: bool,
        shift: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() {
            return;
        }
        if shift && let Some(anchor) = self.anchor {
            let order = self.order();
            if let (Some(a), Some(b)) = (
                order.iter().position(|r| *r == anchor),
                order.iter().position(|r| *r == id),
            ) {
                self.select(order[a.min(b)..=a.max(b)].to_vec(), checked, None, w, cx);
            }
        } else {
            self.selection.retain(|r| *r != id);
            if checked {
                self.selection.push(id);
            }
            self.anchor = Some(id);
            cx.notify();
        }
    }
    fn open_inventory(
        &mut self,
        trash: bool,
        cursor: Option<(i64, i64)>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pending || self.editor.is_some() {
            return;
        }
        self.pending = true;
        let (binding, board, executor) = (
            self.binding.clone(),
            self.board,
            cx.background_executor().clone(),
        );
        self.request = Some(cx.spawn_in(w, async move |t, cx| {
            let result = session::recovery(&binding, board, trash, cursor, &executor).await;
            let _ = t.update_in(cx, |t, _, cx| {
                t.pending = false;
                match result {
                    Ok(read) => {
                        if cursor.is_some()
                            && let Some((_, pages)) = &mut t.inventory
                        {
                            pages.push(read);
                        } else {
                            t.inventory = Some((trash, vec![read]));
                        }
                        t.reconcile = false;
                        t.message = "";
                    }
                    Err(e) => {
                        t.reconcile = true;
                        t.report(e, cx);
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn rename(
        &mut self,
        row: Entity<NameRow>,
        intent: &Rename,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (binding, board, executor) = (
            self.binding.clone(),
            self.board,
            cx.background_executor().clone(),
        );
        let (edit, item, retry) = (intent.edit(), intent.item(), intent.is_retry());
        self.saves.retain(|t| !t.is_ready());
        self.saves.push(cx.spawn_in(w, async move |t, cx| {
            let mut accepted = None;
            let read = if retry {
                match session::board(&binding, board, &executor).await {
                    Ok((rows, true)) => {
                        accepted = rows.into_iter().find(|r| r.id == item);
                        if accepted.is_some() {
                            Ok(())
                        } else {
                            Err(EditError::Missing)
                        }
                    }
                    Ok(_) => Err(EditError::PermissionDenied),
                    Err(e) => Err(e),
                }
            } else {
                Ok(())
            };
            let result = match read {
                Ok(()) => session::rename(&binding, board, item, &edit, &executor).await,
                Err(e) => Err(e),
            };
            let denied = matches!(
                result,
                Err(EditError::SessionRejected | EditError::PermissionDenied)
            );
            let _ = t.update_in(cx, |_, _, cx| {
                row.update(cx, |r, cx| r.finished(result, accepted, cx));
                if denied {
                    cx.emit(AccessLost);
                }
                cx.notify();
            });
        }));
    }
    fn group_order(&mut self, id: &str, delta: isize, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || !self.read.is_manageable() {
            return;
        }
        let mut ids: Vec<_> = self
            .read
            .groups()
            .iter()
            .map(|g| g.id().to_owned())
            .collect();
        if let Some(ix) = ids.iter().position(|g| g == id)
            && let Some(to) = ix.checked_add_signed(delta)
            && to < ids.len()
        {
            ids.swap(ix, to);
            self.submit(
                TableCommand::GroupOrder {
                    board_revision: self.read.revision(),
                    ids,
                },
                None,
                None,
                w,
                cx,
            );
        }
    }
}
fn key(value: impl std::fmt::Display) -> SharedString {
    value.to_string().into()
}
fn status(text: impl Into<SharedString>) -> impl IntoElement {
    let text = text.into();
    div()
        .id("table-status")
        .role(Role::Status)
        .aria_label(text.clone())
        .child(text)
}

impl Render for Table {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = div()
            .id("main-table")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .gap_3()
            .capture_key_down(cx.listener(|t, event: &gpui_kit::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    t.drag.borrow_mut().cancel();
                    for drag in t.row_drags.values() {
                        drag.borrow_mut().cancel();
                    }
                    cx.notify();
                }
            }));
        if let Some((trash, pages)) = &self.inventory {
            let trash = *trash;
            body = body.child(status(if trash { "Trash" } else { "Archive" }));
            for (ix, page) in pages.iter().enumerate() {
                if ix == 0 {
                    body = body.child(format!("{} items", page.total()));
                    if page.has_more() {
                        body = body.child("More items exist beyond this bounded inventory.");
                    }
                    for item in page.items() {
                        let (id, revision) = (item.id(), item.revision());
                        body = body.child(
                            div()
                                .flex()
                                .gap_2()
                                .child(item.title().to_owned())
                                .child(item.group_name().to_owned())
                                .child(
                                    Button::new(key(format!("restore-{id}")))
                                        .label("Restore item")
                                        .accessibility_id(id.to_string())
                                        .disabled(
                                            !item.is_restorable() || self.pending || self.reconcile,
                                        )
                                        .on_click(cx.listener(move |t, _, w, cx| {
                                            t.open_editor(
                                                Kind::RestoreRoot(id, revision, trash),
                                                w,
                                                cx,
                                            )
                                        })),
                                ),
                        );
                    }
                }
                for group in page.groups() {
                    let id = group.id().to_owned();
                    body = body.child(
                        div().flex().gap_2().child(group.name().to_owned()).child(
                            Button::new(key(format!("restore-group-{id}")))
                                .label("Restore group")
                                .accessibility_id(id.clone())
                                .disabled(!page.is_manageable() || self.pending || self.reconcile)
                                .on_click(cx.listener(move |t, _, w, cx| {
                                    t.open_editor(Kind::RestoreGroup(id.clone()), w, cx)
                                })),
                        ),
                    );
                }
            }
            if let Some(last) = pages.last()
                && let Some(offset) = last.next_group_offset()
            {
                let cursor = (last.revision(), offset);
                body = body.child(
                    Button::new("more-groups")
                        .label("More archived groups")
                        .disabled(self.pending)
                        .on_click(cx.listener(move |t, _, w, cx| {
                            t.open_inventory(trash, Some(cursor), w, cx)
                        })),
                );
            }
            body = body
                .child(
                    Button::new("reload-inventory")
                        .label("Reload recovery list")
                        .disabled(self.pending)
                        .on_click(
                            cx.listener(move |t, _, w, cx| t.open_inventory(trash, None, w, cx)),
                        ),
                )
                .child(
                    Button::new("close-inventory")
                        .label("Back to board")
                        .disabled(self.pending)
                        .on_click(cx.listener(|t, _, w, cx| {
                            t.inventory = None;
                            t.reload(w, cx);
                        })),
                );
        } else {
            body = body.child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("add-group")
                            .label("New group…")
                            .disabled(self.busy() || !self.read.is_manageable())
                            .on_click(
                                cx.listener(|t, _, w, cx| t.open_editor(Kind::Group(None), w, cx)),
                            ),
                    )
                    .child(
                        Button::new("archive")
                            .label("Archive…")
                            .disabled(self.pending)
                            .on_click(
                                cx.listener(|t, _, w, cx| t.open_inventory(false, None, w, cx)),
                            ),
                    )
                    .child(
                        Button::new("trash")
                            .label("Trash…")
                            .disabled(self.pending)
                            .on_click(
                                cx.listener(|t, _, w, cx| t.open_inventory(true, None, w, cx)),
                            ),
                    ),
            );
            if !self.selection.is_empty() {
                body = body.child(
                    div()
                        .id("selected-actions")
                        .role(Role::Toolbar)
                        .aria_label("Selected item actions")
                        .flex()
                        .gap_2()
                        .child(status(format!("{} selected", self.selection.len())))
                        .child(
                            Button::new("move-selected")
                                .label("Move selected…")
                                .disabled(self.busy() || !self.read.is_editable())
                                .on_click(cx.listener(|t, _, w, cx| {
                                    t.open_editor(Kind::MoveSelection, w, cx)
                                })),
                        )
                        .child(
                            Button::new("archive-selected")
                                .label("Archive selected…")
                                .disabled(self.busy() || !self.read.is_manageable())
                                .on_click(cx.listener(|t, _, w, cx| {
                                    t.open_editor(Kind::ArchiveSelection, w, cx)
                                })),
                        )
                        .child(
                            Button::new("delete-selected")
                                .label("Delete selected…")
                                .disabled(self.busy() || !self.read.is_manageable())
                                .on_click(cx.listener(|t, _, w, cx| {
                                    t.open_editor(Kind::DeleteSelection, w, cx)
                                })),
                        )
                        .child(
                            Button::new("clear-selected")
                                .label("Clear item selection")
                                .disabled(self.pending)
                                .on_click(cx.listener(|t, _, _, cx| {
                                    t.selection.clear();
                                    cx.notify();
                                })),
                        ),
                );
            }
            body = body.child(self.render_groups(cx));
        }
        body.child(status(self.message)).child(
            Button::new("reload-board")
                .label("Reload board")
                .disabled(self.pending || self.editor.is_some())
                .on_click(cx.listener(|t, _, w, cx| t.reload(w, cx))),
        )
    }
}
