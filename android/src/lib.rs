#![cfg(target_os = "android")]

mod connection;
mod login;
mod navigation;
mod realtime;
mod session;
mod session_record;
mod session_store;

use boardcraft_mobile_edit_client::{EditClient, EditError, NameEdit, SavedItem};
use boardcraft_ui::{BoardFindDock, BoardHeader, BoardRow, BoardTabs, BoardToolbar};
use connection::{Connection, launch_connection};
use gpui_kit::base::Selectable;
use gpui_kit::component::button::ButtonVariants;
use gpui_kit::component::{
    Disableable, Root, Sizable, Theme, ThemeMode,
    button::Button,
    input::{Input, InputEvent, InputState},
    label::Label,
};
use gpui_kit::*;
use gpui_mobile::android::{host, jni};
use login::Login;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use uuid::Uuid;

#[derive(Deserialize)]
struct Item {
    id: Uuid,
    title: String,
    // Absent on the offline, display-only fixture.
    revision: Option<i64>,
}

impl From<SavedItem> for Item {
    fn from(row: SavedItem) -> Self {
        Self {
            id: row.id,
            title: row.title,
            revision: Some(row.revision),
        }
    }
}

#[derive(Deserialize)]
struct Board {
    title: String,
    items: Vec<Item>,
}

struct BoardView {
    board: Board,
    connection: Option<Connection>,
    client: Option<EditClient>,
    login: Option<Login>,
    navigation: Option<navigation::Page>,
    /// The signed-in session's accepted-change feed; None on the login screen.
    changes: Option<realtime::Changes>,
    navigation_focus: FocusHandle,
    /// Unread inbox rows for the board list; None until the count is read.
    inbox_unread: Option<i64>,
    inbox_count_pending: bool,
    logging_out: bool,
    message: &'static str,
    editor: Option<Editor>,
    automation_name: Entity<InputState>,
    automation_pending: bool,
    board_search: Entity<InputState>,
    _board_search_events: Subscription,
    search_task: Option<Task<()>>,
    result_search: String,
    search_pending: bool,
    search_invalid: bool,
    search_focused: bool,
    search_focus: FocusHandle,
    _search_focus_events: Subscription,
    folded: HashMap<String, bool>,
    selected: HashSet<Uuid>,
    table_scroll: ScrollHandle,
    navigation_title: String,
    views_open: bool,
    automations_open: bool,
    filters_open: bool,
    filter_focus: FocusHandle,
    filter_groups: HashSet<String>,
    filter_status: HashSet<String>,
    filter_blank_status: bool,
}

struct Editor {
    id: Uuid,
    original: String,
    revision: Option<i64>,
    input: Entity<InputState>,
    pending: bool,
    error: Option<&'static str>,
    retry: bool,
    _changes: Subscription,
}

impl BoardView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let automation_name =
            cx.new(|cx| InputState::new(window, cx).placeholder("Automation name"));
        let board_search = cx.new(|cx| InputState::new(window, cx).placeholder("Quick find"));
        let board_search_events = cx.subscribe_in(
            &board_search,
            window,
            |view: &mut Self, _, event, w, cx| match event {
                InputEvent::Change => view.search_changed(w, cx),
                InputEvent::Focus => {
                    view.search_focused = true;
                    cx.notify();
                }
                // Keep Cancel mounted while focus moves from the input to a dock action.
                InputEvent::Blur => {}
                InputEvent::PressEnter { .. } => {
                    view.search_task = None;
                    view.apply_search(cx);
                    w.focus(&view.navigation_focus, cx);
                }
            },
        );
        let search_focus = cx.focus_handle();
        let owner = cx.weak_entity();
        let search_focus_events = window.on_focus_out(&search_focus, cx, move |_, _, cx| {
            let _ = owner.update(cx, |view, cx| {
                view.search_focused = false;
                cx.notify();
            });
        });
        let mut view = Self {
            board: Board {
                title: "Sample board".into(),
                items: Vec::new(),
            },
            connection: None,
            client: None,
            login: None,
            navigation: None,
            changes: None,
            navigation_focus: cx.focus_handle(),
            inbox_unread: None,
            inbox_count_pending: false,
            logging_out: false,
            message: "",
            editor: None,
            automation_name,
            automation_pending: false,
            board_search,
            _board_search_events: board_search_events,
            search_task: None,
            result_search: String::new(),
            search_pending: false,
            search_invalid: false,
            search_focused: false,
            search_focus,
            _search_focus_events: search_focus_events,
            folded: HashMap::new(),
            selected: HashSet::new(),
            table_scroll: ScrollHandle::new(),
            navigation_title: "Board".into(),
            views_open: false,
            automations_open: false,
            filters_open: false,
            filter_focus: cx.focus_handle().tab_stop(true),
            filter_groups: HashSet::new(),
            filter_status: HashSet::new(),
            filter_blank_status: false,
        };
        match launch_connection() {
            Ok(None) => {
                view.board = serde_json::from_str(include_str!(
                    "../../scripts/fixtures/android-kit-board.json"
                ))
                .expect("valid synthetic board fixture")
            }
            Err(_) => view.message = "Invalid board connection",
            Ok(Some(conn)) => {
                view.connection = Some(conn);
                view.restore(conn, window, cx);
            }
        }
        view
    }

    fn search_changed(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.search_pending = true;
        self.search_task = Some(cx.spawn_in(w, async move |view, cx| {
            // BoardScreen owns 250ms; the MAUI dock coalesces native edits for 100ms first.
            cx.background_executor()
                .timer(Duration::from_millis(350))
                .await;
            let _ = view.update(cx, |view, cx| view.apply_search(cx));
        }));
        cx.notify();
    }

    fn apply_search(&mut self, cx: &mut Context<Self>) {
        let query = self.board_search.read(cx).value();
        self.search_invalid = query.encode_utf16().count() > 240;
        self.search_pending = false;
        if !self.search_invalid {
            self.result_search = query.trim().to_owned();
            if let Some(navigation::Page::Selected { filter, .. }) = &mut self.navigation {
                *filter = self.result_search.clone();
            }
        }
        cx.notify();
    }

    fn clear_search(&mut self, cancel: bool, w: &mut Window, cx: &mut Context<Self>) {
        self.search_task = None;
        self.board_search
            .update(cx, |input, cx| input.set_value("", w, cx));
        self.apply_search(cx);
        if cancel {
            self.search_focused = false;
            w.focus(&self.navigation_focus, cx);
        } else {
            // Clear disappears with the query; keep focus inside the dock on its input.
            self.board_search.update(cx, |input, cx| input.focus(w, cx));
        }
    }

    fn search_dock(&self, busy: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let mut dock = BoardFindDock::new(&self.board_search)
            .track_focus(&self.search_focus)
            .busy(self.search_pending)
            .disabled(busy)
            .focused(self.search_focused)
            .clear(
                Button::new("mobile-find-clear")
                    .on_click(cx.listener(|view, _, w, cx| view.clear_search(false, w, cx))),
            )
            .cancel(
                Button::new("mobile-find-cancel")
                    .on_click(cx.listener(|view, _, w, cx| view.clear_search(true, w, cx))),
            );
        if self.navigation.is_some() {
            if matches!(self.navigation, Some(navigation::Page::Selected { .. })) {
                dock = dock.filters(
                    Button::new("mobile-board-filters")
                        .track_focus(&self.filter_focus)
                        .selected(
                            self.filters_open
                                || !self.filter_groups.is_empty()
                                || !self.filter_status.is_empty()
                                || self.filter_blank_status,
                        )
                        .disabled(busy)
                        .on_click(cx.listener(|view, _, w, cx| {
                            view.open_filters(w, cx);
                        })),
                );
            }
            dock = dock.home(
                Button::new("mobile-find-home")
                    .disabled(busy)
                    .on_click(cx.listener(|view, _, w, cx| view.navigate(None, w, cx))),
            );
        }
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .children(
                self.search_invalid
                    .then(|| Label::new("Search must be 240 characters or fewer.")),
            )
            .child(dock)
    }

    fn open_editor(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        if self.logging_out
            || self.login.is_some()
            || self.editor.is_some()
            || self.automation_pending
        {
            return;
        }
        let item = match &self.navigation {
            Some(navigation::Page::Selected { rows, status, .. })
                if status.editable && status.pending.is_none() =>
            {
                rows.iter()
                    .find(|row| row.id == id)
                    .map(|row| (row.title.clone(), Some(row.revision)))
            }
            None => self
                .board
                .items
                .iter()
                .find(|item| item.id == id)
                .map(|item| (item.title.clone(), item.revision)),
            _ => None,
        };
        if let Some((title, revision)) = item {
            let input = cx.new(|cx| InputState::new(window, cx).default_value(title.clone()));
            let changes = cx.subscribe_in(&input, window, |view, _, event, w, cx| match event {
                InputEvent::Change => {
                    if let Some(e) = &mut view.editor {
                        e.error = None;
                        cx.notify();
                    }
                }
                InputEvent::PressEnter { .. } => view.save(w, cx),
                _ => {}
            });
            input.update(cx, |input, cx| input.focus(window, cx));
            self.editor = Some(Editor {
                id,
                original: title,
                revision,
                input,
                pending: false,
                error: None,
                retry: false,
                _changes: changes,
            });
            cx.notify();
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let browse = match &self.navigation {
            Some(navigation::Page::Selected { board, status, .. }) if status.editable => {
                Some(*board)
            }
            Some(_) => return,
            None => None,
        };
        let (Some(conn), Some(client), Some(editor)) =
            (self.connection, self.client.clone(), self.editor.as_mut())
        else {
            return;
        };
        let Some(revision) = editor.revision else {
            return;
        };
        let Some(board) = browse.or(conn.board) else {
            return;
        };
        if editor.pending || self.logging_out {
            return;
        }
        let id = editor.id;
        let mut request = NameEdit {
            revision,
            title: editor.input.read(cx).value().to_string(),
        };
        let blank = request.title.trim().is_empty();
        let retry = editor.retry;
        let unchanged = editor.original == request.title;
        if unchanged && !retry {
            self.cancel_editor(window, cx);
            return;
        }
        editor.pending = true;
        editor.error = None;
        window.focus(&self.navigation_focus, cx);
        let identifier = Login::identifier(window, cx);
        let work = cx.background_executor().spawn(async move {
            if browse.is_some() {
                // An explicit retry reconciles the accepted revision before writing the
                // retained draft. A failed read never causes a blind repeat submission.
                if retry {
                    request.revision = client.read_main_table_item(board, id)?.revision;
                }
                client.rename_main_table_item(board, id, &request)
            } else {
                client.rename(board, id, &request)
            }
        });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            _ = view.update(cx, |view, cx| {
                let Some(editor) = view.editor.as_mut().filter(|e| e.id == id && e.pending) else {
                    return;
                };
                editor.pending = false;
                match result {
                    Ok(row) if row.id == id => {
                        if let Some(navigation::Page::Selected {
                            board: current,
                            rows,
                            ..
                        }) = &mut view.navigation
                        {
                            if Some(*current) == browse
                                && let Some(item) = rows.iter_mut().find(|item| item.id == id)
                                && row.revision >= item.revision
                            {
                                *item = row;
                            }
                        } else if let Some(item) =
                            view.board.items.iter_mut().find(|item| item.id == id)
                        {
                            *item = row.into();
                        }
                        view.editor = None;
                    }
                    Err(error @ (EditError::SessionRejected | EditError::PermissionDenied)) => {
                        if error == EditError::SessionRejected {
                            view.forget_session(conn, identifier, cx);
                        } else {
                            view.message = "No permission to edit this board.";
                            view.editor = None;
                            if browse.is_some() {
                                view.navigation = Some(navigation::Page::Denied);
                            } else {
                                view.board.items.clear();
                            }
                        }
                    }
                    result => {
                        editor.retry = browse.is_some();
                        editor.error = Some(match result {
                            Err(EditError::InvalidDraft) if blank => "Name required",
                            Err(EditError::InvalidDraft) => "Name too long",
                            Err(EditError::Conflict) => {
                                "Name changed elsewhere. Retry change to apply your name."
                            }
                            Err(EditError::Missing) => "Item not found. Reopen the board.",
                            Err(EditError::AccessUnavailable) => {
                                "Access check unavailable. Try again."
                            }
                            _ => "Save failed. Try again.",
                        })
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn cancel_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.as_ref().is_some_and(|editor| editor.pending) {
            return;
        }
        self.editor = None;
        window.focus(&self.navigation_focus, cx);
        cx.notify();
    }

    fn render_name(&self, row: &SavedItem, editable: bool, cx: &mut Context<Self>) -> AnyElement {
        let id = row.id;
        if let Some(editor) = self.editor.as_ref().filter(|editor| editor.id == id) {
            return div()
                .flex_1()
                .min_w_0()
                .capture_action(cx.listener(
                    |view, _: &gpui_kit::component::input::Escape, w, cx| {
                        view.cancel_editor(w, cx);
                        cx.stop_propagation();
                    },
                ))
                .child(
                    Input::new(&editor.input)
                        .large()
                        .disabled(editor.pending)
                        .aria_label("Item name"),
                )
                .into_any_element();
        }
        div()
            .flex_1()
            .min_w_0()
            .on_key_down(cx.listener(move |view, event: &KeyDownEvent, w, cx| {
                if event.keystroke.key == "f2" {
                    view.open_editor(id, w, cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                Button::new(SharedString::from(format!("mobile-name-{id}")))
                    .ghost()
                    .large()
                    .flex_1()
                    .min_w_0()
                    .w_full()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_left()
                            .truncate()
                            .child(row.title.clone()),
                    )
                    .accessibility_label(format!("Rename {}", row.title))
                    .disabled(
                        !editable
                            || self.logging_out
                            || self.editor.is_some()
                            || self.automation_pending,
                    )
                    .on_click(cx.listener(move |view, _, w, cx| view.open_editor(id, w, cx))),
            )
            .into_any_element()
    }

    fn editor_actions(&self, editor: &Editor, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .py_2()
            .children(editor.error.map(Label::new))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("mobile-save-name")
                            .large()
                            .label(if editor.pending {
                                "Saving…"
                            } else if editor.retry {
                                "Retry change"
                            } else {
                                "Save"
                            })
                            .disabled(editor.pending)
                            .on_click(cx.listener(|view, _, w, cx| view.save(w, cx))),
                    )
                    .child(
                        Button::new("mobile-cancel-name")
                            .large()
                            .label("Cancel")
                            .disabled(editor.pending)
                            .on_click(cx.listener(|view, _, w, cx| view.cancel_editor(w, cx))),
                    ),
            )
    }

    fn logout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Serialize board operations: a save must settle before revocation, and
        // no new edit or save can start while the logout request is in flight.
        if (self.login.is_some() && !matches!(self.login, Some(Login::Denied)))
            || self.logging_out
            || self.navigation.as_ref().is_some_and(|page| page.pending())
            || self.editor.is_some()
            || self.automation_pending
        {
            return;
        }
        let (Some(client), Some(conn)) = (self.client.clone(), self.connection) else {
            return;
        };
        self.logging_out = true;
        self.message = "";
        let identifier = Login::identifier(window, cx);
        let work = cx
            .background_executor()
            .spawn(async move { client.logout() });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            _ = view.update(cx, |view, cx| {
                view.logging_out = false;
                if result.is_ok() {
                    view.forget_session(conn, identifier, cx);
                } else {
                    view.message = "Could not log out";
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn logout_button(&self, cx: &mut Context<Self>) -> Button {
        Button::new("logout")
            .large()
            .label("Log out")
            .disabled(
                self.logging_out
                    || self.navigation.as_ref().is_some_and(|page| page.pending())
                    || self.editor.is_some()
                    || self.automation_pending,
            )
            .on_click(cx.listener(|view, _, w, cx| view.logout(w, cx)))
    }
}

impl Render for BoardView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut column = div().flex().flex_col().size_full().p_4().gap_3();
        // The feed carries one session's token; a sign-out drops it so the next
        // sign-in opens a feed for the new token.
        if self.client.is_none() {
            self.changes = None;
        }
        if let Some(login) = &self.login {
            return login.form(column, self.message, cx);
        }
        if matches!(self.navigation, Some(navigation::Page::List(_))) {
            self.ensure_inbox_count(cx);
        } else if self.navigation.is_none() {
            // Signing out or opening a board single view drops the badge; the
            // next board list reads it again for the current session.
            self.inbox_unread = None;
        }
        self.ensure_changes(cx);
        if let Some(page) = &self.navigation {
            return self.navigation_page(page, column, window, cx);
        }
        column = column
            .track_focus(&self.navigation_focus)
            .child(BoardHeader::new(self.board.title.clone()))
            .child(BoardTabs::new());
        if !self.message.is_empty() {
            column = column.child(Label::new(self.message));
        }
        let query = self.result_search.to_lowercase();
        let mut table = div()
            .id("mobile-main-table")
            .flex()
            .flex_col()
            .overflow_x_scroll()
            .child(BoardRow::new(Label::new("Item")).header().touch());
        for item in self.board.items.iter().filter(|item| {
            item.title.to_lowercase().contains(&query)
                || self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.id == item.id)
        }) {
            if let Some(editor) = self.editor.as_ref().filter(|e| e.id == item.id) {
                table = table.child(
                    BoardRow::new(
                        Input::new(&editor.input)
                            .large()
                            .disabled(editor.pending || self.logging_out),
                    )
                    .touch(),
                );
                if self.connection.is_some() {
                    table = table.child(
                        Button::new("save")
                            .large()
                            .disabled(editor.pending || self.logging_out)
                            .label(if editor.pending { "Saving…" } else { "Save" })
                            .on_click(cx.listener(|view, _, window, cx| view.save(window, cx))),
                    );
                    if let Some(error) = editor.error {
                        table = table.child(Label::new(error));
                    }
                }
            } else {
                let id = item.id;
                table = table.child(
                    BoardRow::new(
                        Button::new(SharedString::from(id.to_string()))
                            .ghost()
                            .large()
                            .w_full()
                            .disabled(self.logging_out)
                            .child(div().flex_1().min_w_0().child(item.title.clone()))
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.open_editor(id, window, cx)
                            })),
                    )
                    .touch(),
                );
            }
        }
        column = column.child(
            div()
                .id("sample-table-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(table),
        );
        if self.client.is_some() {
            column = column.child(self.logout_button(cx));
        }
        column.child(self.search_dock(self.logging_out || self.editor.is_some(), cx))
    }
}

#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    host::set_gpu_backend(host::GpuBackend::Gl);
    jni::install_panic_hook();
    let _platform = jni::init_platform(&app);
    let platform = jni::shared_platform().expect("Android platform initialized");
    Application::with_platform(platform.into_rc()).run(|cx| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Light, None, cx);
        cx.open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(|cx| BoardView::new(window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("open board fixture window");
        cx.activate(true);
    });
}
