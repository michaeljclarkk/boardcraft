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
    base::Disableable,
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
#[cfg(all(test, not(target_family = "wasm")))]
mod tests;
use editor::{Editor, Kind, Phase};

pub(super) struct AccessLost;
impl EventEmitter<AccessLost> for Table {}
struct Entry {
    input: Entity<InputState>,
    proposed: Uuid,
    notice: &'static str,
    uncertain: bool,
    _events: Subscription,
}
pub(super) struct Table {
    binding: String,
    board: Uuid,
    read: MainTableRead,
    rows: HashMap<Uuid, Entity<NameRow>>,
    entries: HashMap<String, Entry>,
    selection: Vec<Uuid>,
    selection_resolved: bool,
    anchor: Option<Uuid>,
    folds: HashMap<String, bool>,
    pending: bool,
    reconcile: bool,
    message: &'static str,
    editor: Option<Entity<Editor>>,
    inventory: Option<(bool, Vec<ArchiveRead>)>,
    undo_group: Option<(String, String, i64)>,
    drag: Rc<RefCell<HeadingDrag>>,
    row_drags: HashMap<String, Rc<RefCell<HeadingDrag>>>,
    focus: FocusHandle,
    scroll: gpui_kit::ScrollHandle,
    request: Option<Task<()>>,
    saves: Vec<Task<()>>,
    subscriptions: Vec<Subscription>,
}
impl Table {
    pub(super) fn new(
        binding: String,
        board: Uuid,
        read: MainTableRead,
        scroll: gpui_kit::ScrollHandle,
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
            selection_resolved: false,
            anchor: None,
            folds: HashMap::new(),
            pending: false,
            reconcile: false,
            message: "",
            editor: None,
            inventory: None,
            undo_group: None,
            drag: Rc::new(RefCell::new(HeadingDrag::default())),
            row_drags: HashMap::new(),
            focus: cx.focus_handle(),
            scroll,
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
        self.selection_resolved = false;
        if self.read.is_editable() {
            self.folds.clear();
        }
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
                        notice: "",
                        uncertain: false,
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
    fn report(&mut self, error: &EditError, cx: &mut Context<Self>) {
        self.message = match error {
            EditError::Conflict => "Board changed elsewhere. Reload before choosing again.",
            EditError::InvalidDraft => "Change rejected. Correct the draft.",
            EditError::PermissionDenied => "Change denied. Reload to check current access.",
            EditError::Missing => "Record unavailable. Reload the board.",
            EditError::SessionRejected => "Session ended",
            EditError::HostFault | EditError::AccessUnavailable => {
                "Change refused. Try again or reload the board."
            }
            _ => "Result unconfirmed. Reload before choosing again.",
        };
        if error == &EditError::SessionRejected {
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
                        t.undo_group = None;
                        for entry in t.entries.values_mut().filter(|e| e.uncertain) {
                            entry.uncertain = false;
                            if read.groups().iter().flat_map(|g| g.items()).any(|r| r.id == entry.proposed) {
                                entry.notice = "The proposed item is on the board. Start a new draft to add another.";
                                entry.input.update(cx, |i,cx| i.set_value("",w,cx));
                                entry.proposed = Uuid::new_v4();
                            } else {
                                entry.notice = "The proposed item is not on the active board. Check the draft before retrying.";
                            }
                        }
                        t.read = read;
                        t.reconcile = false;
                        t.message = "";
                        t.mount(w, cx);
                    }
                    Err(EditError::PermissionDenied | EditError::Missing) => cx.emit(AccessLost),
                    Err(e) => t.report(&e, cx),
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
        let Some(entry) = self.entries.get_mut(group) else {
            return;
        };
        let title = entry.input.read(cx).value().to_string();
        if title.trim().is_empty() {
            return;
        }
        entry.notice = "Saving item…";
        let command = TableCommand::Create {
            id: entry.proposed,
            group: group.into(),
            title,
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
            let read = if saved.is_ok() || saved == Err(EditError::PermissionDenied) {
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
                        t.undo_group = None;
                        if let Some(group) = entry.as_ref()
                            && let Some(e) = t.entries.get_mut(group)
                        {
                            e.input.update(cx, |i, cx| i.set_value("", w, cx));
                            e.proposed = Uuid::new_v4();
                            e.notice = "";
                        }
                        if matches!(command, TableCommand::Selection { .. }) {
                            t.selection.clear();
                        }
                        if let Some(Ok(read)) = read {
                            if let TableCommand::Group {
                                id,
                                restore: false,
                                board_revision,
                            } = &command
                                && board_revision.checked_add(1) == Some(read.revision())
                                && let Some(group) = t.read.groups().iter().find(|g| g.id() == id)
                            {
                                t.undo_group =
                                    Some((id.clone(), group.name().to_owned(), read.revision()));
                            }
                            t.read = read;
                            t.mount(w, cx);
                            t.reconcile = false;
                        } else {
                            t.reconcile = true;
                            t.message = "Saved, but the board could not be read. Reload board.";
                            if matches!(
                                read,
                                Some(Err(EditError::SessionRejected | EditError::PermissionDenied))
                            ) {
                                cx.emit(AccessLost);
                            }
                        }
                        if let Some(result) = recovery {
                            match result {
                                Ok(r) => t.inventory = Some((inventory.unwrap_or(false), vec![r])),
                                Err(e) => {
                                    t.reconcile = true;
                                    t.report(&e, cx);
                                }
                            }
                        }
                        if editor.is_some() {
                            t.close_editor(w, cx);
                        }
                    }
                    Err(error) => {
                        if error == EditError::PermissionDenied {
                            if editor.is_some() {
                                t.close_editor(w, cx);
                            }
                            t.inventory = None;
                            t.undo_group = None;
                            if let Some(Ok(read)) = read {
                                t.read = read;
                                t.mount(w, cx);
                                t.reconcile = false;
                                t.message = "Change denied. Current permissions are shown.";
                            } else {
                                cx.emit(AccessLost);
                            }
                            cx.notify();
                            return;
                        }
                        let uncertain = matches!(error, EditError::Transport);
                        t.reconcile = uncertain
                            || matches!(error, EditError::PermissionDenied | EditError::Missing);
                        t.report(&error, cx);
                        if let Some(e) = entry.as_ref().and_then(|id| t.entries.get_mut(id)) {
                            e.notice = t.message;
                            e.uncertain = uncertain;
                        }
                        if let Some(e) = editor {
                            let _ = e.update(cx, |e, cx| {
                                e.message = t.message.into();
                                e.phase = if matches!(
                                    error,
                                    EditError::InvalidDraft
                                        | EditError::HostFault
                                        | EditError::AccessUnavailable
                                ) {
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
                        if entry.is_none() && t.editor.is_none() && error == EditError::Conflict {
                            t.reconcile = true;
                        }
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
    fn select(&mut self, ids: Vec<Uuid>, checked: bool, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        if self.selection_resolved {
            self.apply_selection(ids, checked);
            cx.notify();
            return;
        }
        // Resolve the whole gesture or none of it: stale membership could silently
        // change the meaning of an interval even if its two endpoints still exist.
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
                        let same = read.groups().len() == t.read.groups().len()
                            && read.groups().iter().zip(t.read.groups()).all(|(a, b)| {
                                a.id() == b.id()
                                    && a.items()
                                        .iter()
                                        .map(|i| i.id)
                                        .eq(b.items().iter().map(|i| i.id))
                            });
                        t.read = read;
                        t.mount(w, cx);
                        if same {
                            t.apply_selection(ids, checked);
                            t.selection_resolved = true;
                            t.message = "";
                        } else {
                            t.message = "Items changed. Review the board and select again.";
                        }
                    }
                    Err(EditError::PermissionDenied | EditError::Missing) => cx.emit(AccessLost),
                    Err(e) => t.report(&e, cx),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn apply_selection(&mut self, ids: Vec<Uuid>, checked: bool) {
        self.selection.retain(|id| !ids.contains(id));
        if checked {
            self.selection.extend(ids);
        }
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
                self.select(order[a.min(b)..=a.max(b)].to_vec(), checked, w, cx);
            }
        } else {
            self.apply_selection(vec![id], checked);
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
        if self.pending || self.editor.is_some() || (self.reconcile && self.inventory.is_none()) {
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
                        if matches!(e, EditError::PermissionDenied | EditError::Missing) {
                            cx.emit(AccessLost);
                        }
                        t.reconcile = true;
                        t.report(&e, cx);
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
                        let board_revision = page.revision();
                        body = body.child(
                            div()
                                .flex()
                                .gap_2()
                                .child(item.title().to_owned())
                                .child(item.group_name().to_owned())
                                .child(if !item.is_group_active() {
                                    "Restore its group first"
                                } else if !page.is_manageable() {
                                    "Owner access required"
                                } else {
                                    ""
                                })
                                .child(
                                    Button::new(key(format!("restore-{id}")))
                                        .label("Restore item")
                                        .accessibility_id(id.to_string())
                                        .disabled(
                                            !item.is_restorable() || self.pending || self.reconcile,
                                        )
                                        .on_click(cx.listener(move |t, _, w, cx| {
                                            t.submit(
                                                TableCommand::Root {
                                                    id,
                                                    revision,
                                                    board_revision,
                                                    operation: Uuid::new_v4(),
                                                    action: if trash {
                                                        RootAction::RestoreDeleted
                                                    } else {
                                                        RootAction::Restore
                                                    },
                                                },
                                                None,
                                                None,
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
                    let board_revision = page.revision();
                    body = body.child(
                        div().flex().gap_2().child(group.name().to_owned()).child(
                            Button::new(key(format!("restore-group-{id}")))
                                .label("Restore group")
                                .accessibility_id(id.clone())
                                .disabled(!page.is_manageable() || self.pending || self.reconcile)
                                .on_click(cx.listener(move |t, _, w, cx| {
                                    t.submit(
                                        TableCommand::Group {
                                            id: id.clone(),
                                            board_revision,
                                            restore: true,
                                        },
                                        None,
                                        None,
                                        w,
                                        cx,
                                    )
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
                        .disabled(self.busy())
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
                            .disabled(self.busy())
                            .on_click(
                                cx.listener(|t, _, w, cx| t.open_inventory(false, None, w, cx)),
                            ),
                    )
                    .child(
                        Button::new("trash")
                            .label("Trash…")
                            .disabled(self.busy())
                            .on_click(
                                cx.listener(|t, _, w, cx| t.open_inventory(true, None, w, cx)),
                            ),
                    ),
            );
            if let Some((id, name, revision)) = self.undo_group.clone() {
                body =
                    body.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(format!("{name} was archived"))
                            .child(
                                Button::new("undo-group")
                                    .label("Undo")
                                    .disabled(self.busy() || !self.read.is_manageable())
                                    .on_click(cx.listener(move |t, _, w, cx| {
                                        t.submit(
                                            TableCommand::Group {
                                                id: id.clone(),
                                                board_revision: revision,
                                                restore: true,
                                            },
                                            None,
                                            None,
                                            w,
                                            cx,
                                        );
                                    })),
                            )
                            .child(Button::new("dismiss-undo").label("Dismiss").on_click(
                                cx.listener(|t, _, _, cx| {
                                    t.undo_group = None;
                                    cx.notify();
                                }),
                            )),
                    );
            }
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
        body.child(status(if self.pending {
            "Working…"
        } else {
            self.message
        }))
        .child(
            Button::new("reload-board")
                .label("Reload board")
                .disabled(self.pending || self.editor.is_some() || self.inventory.is_some())
                .on_click(cx.listener(|t, _, w, cx| t.reload(w, cx))),
        )
    }
}
