//! A selected-board visit: drafts and gestures here, accepted records at the host.
use crate::{
    board_view::{NameRow, Rename},
    session,
};
use boardcraft_mobile_edit_client::{
    ArchiveRead, EditError, ItemReorder, MainTableRead, RootAction, SelectionAction, TableCommand,
    ViewFilters,
};
use boardcraft_ui::{BoardSelectionBar, BoardToolbar, HeadingDrag};
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Render, Role, SharedString, StatefulInteractiveElement, Styled, Subscription,
    Task, TestSupportExt, WeakEntity, Window,
    assets::IconName,
    base::Disableable,
    component::{
        ActiveTheme, Sizable, WindowExt,
        button::Button,
        checkbox::Checkbox,
        input::{Input, InputEvent, InputState},
        menu::{DropdownMenu, PopupMenuItem},
    },
    div,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};
use uuid::Uuid;
mod editor;
mod gestures;
mod rename;
mod status_column;
#[cfg(all(test, not(target_family = "wasm")))]
mod tests;
mod views;
use editor::{Editor, Kind, Phase};
use rename::FailedRename;
use status_column::StatusColumn;

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
    status: StatusColumn,
    view_filters: ViewFilters,
    filter_dialog: Option<Entity<views::FilterDialog>>,
    saved_views: Option<Entity<views::ViewsDialog>>,
    views_open: bool,
    filter_focus: FocusHandle,
    views_focus: FocusHandle,
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
    archive_focus: FocusHandle,
    delete_focus: FocusHandle,
    move_focus: FocusHandle,
    scroll: gpui_kit::ScrollHandle,
    request: Option<Task<()>>,
    saves: Vec<Task<()>>,
    failed_renames: Vec<FailedRename>,
    subscriptions: Vec<Subscription>,
    search: Entity<InputState>,
    result_search: String,
    search_invalid: bool,
    search_task: Option<Task<()>>,
    _search_events: Subscription,
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
        let search = cx.new(|cx| InputState::new(w, cx).placeholder("Search"));
        let search_events = cx.subscribe_in(&search, w, |t, _, event, w, cx| {
            if matches!(event, InputEvent::Change) {
                t.search_changed(w, cx);
            }
        });
        let mut table = Self {
            binding,
            board,
            read,
            status: StatusColumn::new(cx),
            view_filters: ViewFilters::default(),
            filter_dialog: None,
            saved_views: None,
            views_open: false,
            filter_focus: cx.focus_handle().tab_stop(true),
            views_focus: cx.focus_handle().tab_stop(true),
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
            archive_focus: cx.focus_handle().tab_stop(true),
            delete_focus: cx.focus_handle().tab_stop(true),
            move_focus: cx.focus_handle().tab_stop(true),
            scroll,
            request: None,
            saves: Vec::new(),
            failed_renames: Vec::new(),
            subscriptions: Vec::new(),
            search,
            result_search: String::new(),
            search_invalid: false,
            search_task: None,
            _search_events: search_events,
        };
        table.mount(w, cx);
        table
    }
    fn search_changed(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let query = self.search.read(cx).value().to_string();
        self.search_task = Some(cx.spawn_in(w, async move |table, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let _ = table.update_in(cx, |table, _, cx| {
                table.search_invalid = query.encode_utf16().count() > 240;
                if !table.search_invalid {
                    table.result_search = query.trim().to_lowercase();
                    // An old drag's hit regions must not survive a changed result set.
                    table.drag.borrow_mut().cancel();
                    for drag in table.row_drags.values() {
                        drag.borrow_mut().cancel();
                    }
                }
                cx.notify();
            });
        }));
    }

    fn matches_search(&self, id: Uuid, cx: &App) -> bool {
        self.matches_view_filters(id)
            && (self.result_search.is_empty()
                || self.rows.get(&id).is_some_and(|row| {
                    row.read(cx)
                        .accepted_title()
                        .to_lowercase()
                        .contains(&self.result_search)
                }))
    }

    fn matching_order(&self, cx: &App) -> Vec<Uuid> {
        self.order()
            .into_iter()
            .filter(|id| self.matches_search(*id, cx))
            .collect()
    }

    fn new_item_available(&self, cx: &App) -> bool {
        !self.busy()
            && self.read.is_editable()
            && ((self.result_search.is_empty() && self.view_filters == ViewFilters::default())
                || !self.matching_order(cx).is_empty())
            && self.read.groups().iter().any(|group| {
                !self.folded(group.id(), group.is_collapsed())
                    && (self.view_filters.group_ids.is_empty()
                        || self
                            .view_filters
                            .group_ids
                            .iter()
                            .any(|id| id == group.id()))
                    && self.entries.contains_key(group.id())
            })
    }

    fn focus_new_item(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if !self.new_item_available(cx) {
            return;
        }
        if let Some(group) = self.read.groups().iter().find(|group| {
            !self.folded(group.id(), group.is_collapsed())
                && (self.view_filters.group_ids.is_empty()
                    || self
                        .view_filters
                        .group_ids
                        .iter()
                        .any(|id| id == group.id()))
        }) && let Some(entry) = self.entries.get(group.id())
        {
            entry.input.update(cx, |input, cx| input.focus(w, cx));
        }
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let group = cx.weak_entity();
        let tools = cx.weak_entity();
        let manageable = self.read.is_manageable();
        let busy = self.busy();
        BoardToolbar::new()
            .search_disabled(busy)
            .new_item(
                Button::new("new-item")
                    .disabled(busy || !self.read.is_editable())
                    .on_click(cx.listener(|t, _, w, cx| t.focus_new_item(w, cx))),
            )
            .new_item_options(
                BoardToolbar::new_item_options_trigger("new-item-options")
                    .disabled(busy)
                    .dropdown_menu(move |menu, _, _| {
                        let group = group.clone();
                        menu.item(
                            PopupMenuItem::new("New group of items")
                                .disabled(!manageable)
                                .on_click(move |_, w, cx| {
                                    let _ = group.update(cx, |t, cx| {
                                        t.open_editor(Kind::Group(None), w, cx)
                                    });
                                }),
                        )
                    }),
            )
            .search(&self.search, "board-search-input")
            .action(
                Button::new("board-filter")
                    .label("Filter")
                    .track_focus(&self.filter_focus)
                    .disabled(busy)
                    .on_click(cx.listener(|table, _, w, cx| table.open_filters(w, cx))),
            )
            .action(
                Button::new("board-views")
                    .label("Views")
                    .track_focus(&self.views_focus)
                    .disabled(busy)
                    .on_click(cx.listener(|table, _, w, cx| table.open_views(w, cx))),
            )
            .tools(
                BoardToolbar::tools_trigger("board-tools-menu")
                    .disabled(self.pending || self.editor.is_some())
                    .dropdown_menu(move |menu, _, _| {
                        let add = tools.clone();
                        let reload = tools.clone();
                        let recovery = tools.clone();
                        menu.item(
                            PopupMenuItem::new("Add group")
                                .disabled(busy || !manageable)
                                .on_click(move |_, w, cx| {
                                    let _ = add.update(cx, |t, cx| {
                                        t.open_editor(Kind::Group(None), w, cx)
                                    });
                                }),
                        )
                        .item(
                            PopupMenuItem::new("Reload board").on_click(move |_, w, cx| {
                                let _ = reload.update(cx, |t, cx| t.reload(w, cx));
                            }),
                        )
                        .item(
                            PopupMenuItem::new("Archived & deleted content")
                                .disabled(busy)
                                .on_click(move |_, w, cx| {
                                    let _ = recovery
                                        .update(cx, |t, cx| t.open_inventory(false, None, w, cx));
                                }),
                        )
                    }),
            )
    }

    fn selection_bar(&self, cx: &mut Context<Self>) -> BoardSelectionBar {
        BoardSelectionBar::new(self.selection.len())
            .status_controls(self.bulk_status_controls(cx))
            .archive(
                Button::new("archive-selected")
                    .track_focus(&self.archive_focus)
                    .disabled(self.busy() || !self.read.is_manageable())
                    .on_click(cx.listener(|t, _, w, cx| {
                        t.archive_focus.focus(w, cx);
                        t.open_editor(Kind::ArchiveSelection, w, cx)
                    })),
            )
            .delete(
                Button::new("delete-selected")
                    .track_focus(&self.delete_focus)
                    .disabled(self.busy() || !self.read.is_manageable())
                    .on_click(cx.listener(|t, _, w, cx| {
                        t.delete_focus.focus(w, cx);
                        t.open_editor(Kind::DeleteSelection, w, cx)
                    })),
            )
            .move_to(
                Button::new("move-selected")
                    .track_focus(&self.move_focus)
                    .disabled(self.busy() || !self.read.is_editable())
                    .on_click(cx.listener(|t, _, w, cx| {
                        t.move_focus.focus(w, cx);
                        t.open_editor(Kind::MoveSelection, w, cx)
                    })),
            )
            .clear(
                Button::new("clear-selected")
                    .disabled(self.pending)
                    .on_click(cx.listener(|t, _, w, cx| {
                        t.selection.clear();
                        t.reset_bulk_status_choice();
                        t.search.update(cx, |input, cx| input.focus(w, cx));
                        cx.notify();
                    })),
            )
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
        self.rows
            .retain(|id, row| available.contains(id) || row.read(cx).has_request());
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
                let input = cx.new(|cx| InputState::new(w, cx).placeholder("+ Add item"));
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
                    |t, _, intent: &Rename, w, cx| t.rename(intent.clone(), w, cx),
                ));
            }
        }
        #[cfg(target_family = "wasm")]
        self.load_status(w, cx);
        cx.notify();
    }
    fn busy(&self) -> bool {
        self.pending
            || self.reconcile
            || self.editor.is_some()
            || self.filter_dialog.is_some()
            || self.views_open
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
            let order = self.matching_order(cx);
            if let (Some(a), Some(b)) = (
                order.iter().position(|r| *r == anchor),
                order.iter().position(|r| *r == id),
            ) {
                self.select(order[a.min(b)..=a.max(b)].to_vec(), checked, w, cx);
            } else {
                self.message =
                    "An item left this view. Reload the board and select the range again.";
                cx.notify();
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
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("archive")
                            .small()
                            .label("Archive")
                            .disabled(self.pending || !trash)
                            .on_click(
                                cx.listener(|t, _, w, cx| t.open_inventory(false, None, w, cx)),
                            ),
                    )
                    .child(
                        Button::new("trash")
                            .small()
                            .label("Trash")
                            .disabled(self.pending || trash)
                            .on_click(
                                cx.listener(|t, _, w, cx| t.open_inventory(true, None, w, cx)),
                            ),
                    ),
            );
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
            body = body.child(self.toolbar(cx));
            if self.search_invalid {
                body = body.child(
                    div()
                        .id("board-search-error")
                        .role(Role::Status)
                        .text_color(cx.theme().danger)
                        .child("Search is limited to 240 characters."),
                );
            }
            if !self.result_search.is_empty() || self.view_filters != ViewFilters::default() {
                body = body.child(
                    div()
                        .id("board-match-summary")
                        .role(Role::Status)
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{} matching items", self.matching_order(cx).len())),
                );
            }
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
            for failure in &self.failed_renames {
                body = body.child(self.rename_notice(failure, cx));
            }
            if let Some(notice) = self.status_notice(cx) {
                body = body.child(notice);
            }
            if (!self.result_search.is_empty() || self.view_filters != ViewFilters::default())
                && self.matching_order(cx).is_empty()
            {
                body = body.child(
                    div()
                        .id("board-no-results")
                        .role(Role::Status)
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_3()
                        .py_6()
                        .child(IconName::Search)
                        .child(div().text_lg().child("No results were found"))
                        .child("Try another search or review this view’s filters.")
                        .child(
                            Button::new("clear-board-search")
                                .label(if self.view_filters == ViewFilters::default() {
                                    "Clear search"
                                } else {
                                    "Clear filters"
                                })
                                .on_click(cx.listener(|t, _, w, cx| {
                                    t.search.update(cx, |input, cx| {
                                        input.set_value("", w, cx);
                                        input.focus(w, cx);
                                    });
                                    // Programmatic changes do not emit InputEvent::Change.
                                    t.search_changed(w, cx);
                                    t.view_filters = ViewFilters::default();
                                })),
                        ),
                );
            } else {
                body = body.child(self.render_groups(cx));
            }
        }
        body = body.child(status(if self.pending && !self.status.is_saving() {
            "Working…"
        } else {
            self.message
        }));
        div()
            .id("main-table-viewport")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(
                div()
                    .id("content-scroll")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .overflow_x_scroll()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(
                        div()
                            .px_6()
                            // Leave the final row reachable above the floating actions
                            // without shrinking the table's scrolling viewport.
                            .pb(if self.inventory.is_none() && !self.selection.is_empty() {
                                gpui_kit::rems(7.)
                            } else {
                                gpui_kit::rems(1.5)
                            })
                            .child(body),
                    ),
            )
            .children(
                (self.inventory.is_none() && !self.selection.is_empty()).then(|| {
                    let footer = div()
                        .id("selection-overlay")
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .flex()
                        .px_3()
                        .py_3()
                        .child(self.selection_bar(cx));
                    // Tuesday's compact bar starts beside the collapsed workspace rail.
                    if w.viewport_size().width <= gpui_kit::px(700.) {
                        footer.justify_start()
                    } else {
                        footer.justify_center().pb_6()
                    }
                }),
            )
    }
}
