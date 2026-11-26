//! Drafts belong to their physical group; accepted rows only come from a fresh read.
use super::*;
use gpui_kit::component::input::InputEvent;
use gpui_kit::prelude::FluentBuilder;

pub(super) struct Create;

pub(super) struct GroupEntry {
    group: SharedString,
    label: SharedString,
    input: Entity<InputState>,
    proposed: Uuid,
    pub(super) enabled: bool,
    error: bool,
    _events: Subscription,
}

impl EventEmitter<Create> for GroupEntry {}

impl GroupEntry {
    pub(super) fn new(group: &str, name: &str, w: &mut Window, cx: &mut Context<Self>) -> Self {
        let label: SharedString = format!("Add item to {name}").into();
        let input = cx.new(|cx| InputState::new(w, cx).placeholder("Item name"));
        let events = cx.subscribe_in(&input, w, |e, _, event, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                e.submit(cx);
            }
        });
        Self {
            group: group.to_owned().into(),
            label,
            input,
            proposed: Uuid::new_v4(),
            enabled: true,
            error: false,
            _events: events,
        }
    }
    pub(super) fn set_group_name(&mut self, name: &str) {
        self.label = format!("Add item to {name}").into();
    }
    fn submit(&self, cx: &mut Context<Self>) {
        if self.enabled && !self.input.read(cx).value().trim().is_empty() {
            cx.emit(Create);
        }
    }
    fn clear(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |i, cx| i.set_value("", w, cx));
        self.proposed = Uuid::new_v4();
        self.error = false;
        cx.notify();
    }
}

impl Render for GroupEntry {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().flex_col().gap_1()
            .capture_action(cx.listener(|e, _: &component::input::Escape, w, cx| {
                if e.enabled { e.clear(w, cx); }
                cx.stop_propagation();
            }))
            .child(Label::new(self.label.clone()))
            .child(div().flex().items_center().gap_2()
                .child(div().flex_1().min_w_0().child(Input::new(&self.input).small()
                    .id(SharedString::from(format!("add-{}", self.group))).aria_label(self.label.clone()).disabled(!self.enabled)))
                .child(Button::new(SharedString::from(format!("submit-{}", self.group))).small()
                    .label("Add item").disabled(!self.enabled).on_click(cx.listener(|e, _, _, cx| e.submit(cx)))))
            .when(self.error, |el| el.child(Label::new("Item not added. Use a nonblank name of at most 255 UTF-16 units, then submit again.")))
    }
}

enum EntryFailure {
    Save(EditError),
    Read(EditError),
}

enum EntryTarget {
    Create(Entity<GroupEntry>),
    Rename(Entity<Row>),
}

struct EntryRequest {
    board: Uuid,
    visit: u64,
    target: EntryTarget,
}

impl EntryTarget {
    fn raw(&self, cx: &App) -> String {
        match self {
            Self::Create(entry) => entry.read(cx).input.read(cx).value().to_string(),
            Self::Rename(row) => row.read(cx).edit.request().unwrap_or("").to_owned(),
        }
    }

    fn discard_projection(&self, w: &mut Window, cx: &mut App) {
        match self {
            Self::Create(entry) => entry.update(cx, |entry, cx| entry.clear(w, cx)),
            Self::Rename(row) => row.update(cx, |row, cx| {
                row.edit.discard_unconfirmed();
                cx.notify();
            }),
        }
    }
}

impl MainTable {
    pub(super) fn set_busy(&mut self, pending: bool, cx: &mut App) {
        self.pending = pending;
        let enabled = self.editable && !pending && !self.reconcile;
        for h in &self.headings {
            h.entry.update(cx, |e, cx| {
                e.enabled = enabled;
                cx.notify();
            });
            for r in &h.rows {
                r.update(cx, |r, cx| {
                    r.editable = enabled;
                    cx.notify();
                });
            }
        }
    }
}

impl Connected {
    pub(super) fn create_main_table_item(
        &mut self,
        entry: Entity<GroupEntry>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, table) = &mut nav.page else {
            return;
        };
        if table.pending
            || table.reconcile
            || !table.editable
            || !table
                .headings
                .iter()
                .any(|h| h.entry == entry && !h.collapsed)
        {
            return;
        }
        let e = entry.read(cx);
        let (group, proposed, raw) = (
            e.group.to_string(),
            e.proposed,
            e.input.read(cx).value().to_string(),
        );
        if raw.trim().is_empty() {
            return;
        }
        let board = *board;
        let client = nav.client.clone();
        table.set_busy(true, cx);
        self.message = "Adding item…";
        #[cfg(all(windows, feature = "native-smoke"))]
        {
            self.submissions += 1;
        }
        let request = EntryRequest {
            board,
            visit: self.navigation_request,
            target: EntryTarget::Create(entry),
        };
        let work = cx.background_executor().spawn(async move {
            client
                .create_main_table_item(board, &group, proposed, &raw)
                .map_err(EntryFailure::Save)?;
            client.read_main_table(board).map_err(EntryFailure::Read)
        });
        cx.spawn_in(w, async move |v, cx| {
            let result = work.await;
            _ = v.update_in(cx, |v, w, cx| v.finish_item_entry(request, result, w, cx));
        })
        .detach();
        cx.notify();
    }

    pub(super) fn rename_main_table_item(
        &mut self,
        row: Entity<Row>,
        save: Save,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, table) = &mut nav.page else {
            return;
        };
        if table.pending
            || table.reconcile
            || !table.editable
            || !table
                .headings
                .iter()
                .any(|h| h.rows.contains(&row) && !h.collapsed)
        {
            return;
        }
        let board = *board;
        let client = nav.client.clone();
        table.set_busy(true, cx);
        self.message = "Saving name…";
        #[cfg(all(windows, feature = "native-smoke"))]
        {
            self.submissions += 1;
        }
        let request = EntryRequest {
            board,
            visit: self.navigation_request,
            target: EntryTarget::Rename(row),
        };
        let work = cx.background_executor().spawn(async move {
            save.send_selected(&client).map_err(EntryFailure::Save)?;
            client.read_main_table(board).map_err(EntryFailure::Read)
        });
        cx.spawn_in(w, async move |v, cx| {
            let result = work.await;
            _ = v.update_in(cx, |v, w, cx| v.finish_item_entry(request, result, w, cx));
        })
        .detach();
        cx.notify();
    }

    fn finish_item_entry(
        &mut self,
        request: EntryRequest,
        result: Result<MainTableRead, EntryFailure>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if request.visit != self.navigation_request {
            return;
        }
        let board = request.board;
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(current, table) = &mut nav.page else {
            return;
        };
        if *current != board || !table.pending {
            return;
        }
        match result {
            Ok(read) => {
                if let EntryTarget::Create(entry) = request.target {
                    entry.update(cx, |entry, cx| entry.clear(w, cx));
                }
                let next = MainTable::new(read, board, Some(table), w, cx);
                nav.page = Page::Items(board, next);
                self.message = "";
            }
            Err(EntryFailure::Save(EditError::InvalidDraft))
                if boardcraft_board::interactive_title(&request.target.raw(cx)).is_err() =>
            {
                match request.target {
                    EntryTarget::Create(entry) => entry.update(cx, |entry, cx| {
                        entry.error = true;
                        cx.notify();
                    }),
                    EntryTarget::Rename(row) => row.update(cx, |row, cx| {
                        let reason =
                            boardcraft_board::interactive_title(row.edit.request().unwrap_or(""))
                                .expect_err("title rejection checked above");
                        row.edit
                            .finish(Err(boardcraft_board::store::SaveError::Rejected(reason)));
                        cx.notify();
                    }),
                }
                table.set_busy(false, cx);
                self.message = "Not saved. Correct the draft before submitting again.";
            }
            Err(
                EntryFailure::Save(EditError::SessionRejected)
                | EntryFailure::Read(EditError::SessionRejected),
            ) => {
                self.link_input = None;
                self.screen = Self::identifier(w, cx);
                self.message = "Session expired. Sign in again.";
            }
            Err(EntryFailure::Read(EditError::PermissionDenied | EditError::Missing)) => {
                nav.page = Page::Unavailable;
                nav.share = None;
                self.message = "Destination unavailable";
            }
            Err(failure) => {
                // Remove a pending projection immediately. Reconciliation is a GET only;
                // the submitted draft must not turn into a second automatic append.
                request.target.discard_projection(w, cx);
                table.reconcile = true;
                table.set_busy(false, cx);
                self.message = match failure {
                    EntryFailure::Save(EditError::PermissionDenied) => {
                        "Not saved: permission denied. Checking current access…"
                    }
                    EntryFailure::Save(EditError::Conflict) => {
                        "Item changed. Reload board and review before editing again."
                    }
                    EntryFailure::Save(EditError::Missing | EditError::InvalidDraft) => {
                        "Item or group unavailable. Reload board before editing again."
                    }
                    _ => {
                        "Save not confirmed. Reload board to check accepted items; the write will not be repeated."
                    }
                };
                if matches!(failure, EntryFailure::Save(EditError::PermissionDenied)) {
                    self.refresh_item_entry(board, w, cx);
                }
            }
        }
        cx.notify();
    }

    pub(in crate::connected) fn refresh_item_entry(
        &mut self,
        board: Uuid,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Screen::Navigation(nav) = &mut self.screen else {
            return false;
        };
        let Page::Items(current, table) = &mut nav.page else {
            return false;
        };
        if *current != board {
            return false;
        }
        if table.pending {
            return true;
        }
        let denied = self.message == "Not saved: permission denied. Checking current access…";
        table.set_busy(true, cx);
        let client = nav.client.clone();
        let request = self.navigation_request;
        let work = cx
            .background_executor()
            .spawn(async move { client.read_main_table(board) });
        cx.spawn_in(w, async move |v, cx| {
            let result = work.await;
            _ = v.update_in(cx, |v, w, cx| {
                if request != v.navigation_request {
                    return;
                }
                let Screen::Navigation(nav) = &mut v.screen else {
                    return;
                };
                let Page::Items(current, old) = &mut nav.page else {
                    return;
                };
                if *current != board {
                    return;
                }
                match result {
                    Ok(read) => {
                        let table = MainTable::new(read, board, Some(old), w, cx);
                        table.focus_first(w, cx);
                        nav.page = Page::Items(board, table);
                        v.message = if denied {
                            "Not saved: permission denied."
                        } else {
                            "Board reloaded. Review accepted items before making another change."
                        };
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
                        v.message = "Could not reload board. Retry loading before editing again.";
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
        true
    }
}
