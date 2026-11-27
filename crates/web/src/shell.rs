use boardcraft_mobile_edit_client::{BoardSummary, Credentials, DISPOSABLE_VIEWER, EditError};
use gpui_kit::{
    AppContext, Context, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    Render, Role, StatefulInteractiveElement, Styled, Subscription, Task, Window,
    base::Disableable,
    component::{
        ActiveTheme, WindowExt,
        button::Button,
        input::{Input, InputEvent, InputState},
    },
    div,
};
use uuid::Uuid;
use wasm_bindgen::{JsCast, closure::Closure};

use crate::{
    binding,
    groups::{AccessLost, Table},
    session,
};

enum Screen {
    Checking,
    Identifier,
    Password,
    SigningIn,
    SignedIn,
    Unavailable,
    StorageUnavailable,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum HostKind {
    Unknown,
    Authenticated,
    Viewer,
}
enum Page {
    Loading,
    List(Vec<BoardSummary>),
    ListUnavailable,
    Board(Entity<Table>),
    Denied,
    BoardUnavailable,
}

pub(super) struct Shell {
    host: HostKind,
    screen: Screen,
    page: Page,
    destination: Option<Uuid>,
    binding: Option<String>,
    identifier: Entity<InputState>,
    password: Entity<InputState>,
    message: &'static str,
    logging_out: bool,
    focus: FocusHandle,
    scroll: gpui_kit::ScrollHandle,
    action_focus: FocusHandle,
    request: Option<Task<()>>,
    saves: Vec<Task<()>>,
    events: Vec<Subscription>,
    changes: Option<web_sys::BroadcastChannel>,
    _changed: Closure<dyn FnMut()>,
    history: Closure<dyn FnMut()>,
}

impl Shell {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let identifier = cx.new(|cx| InputState::new(window, cx).placeholder("Username or email"));
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Password")
                .masked(true)
        });
        let mut events = Vec::new();
        for field in [&identifier, &password] {
            events.push(cx.subscribe_in(field, window, |shell, _, event, w, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    shell.advance(w, cx);
                }
            }));
        }
        let handle = window.window_handle();
        let weak = cx.weak_entity();
        let mut app = cx.to_async();
        let changed = Closure::new(move || {
            let _ = handle.update(&mut app, |_, w, cx| {
                let _ = weak.update(cx, |shell: &mut Self, cx| shell.check(w, cx));
            });
        });
        let weak = cx.weak_entity();
        let mut app = cx.to_async();
        let history = Closure::new(move || {
            let _ = handle.update(&mut app, |_, w, cx| {
                let _ = weak.update(cx, |shell: &mut Self, cx| {
                    shell.destination = address();
                    shell.check(w, cx);
                });
            });
        });
        let changes = binding::changes();
        if let Some(changes) = &changes {
            changes.set_onmessage(Some(changed.as_ref().unchecked_ref()));
        }
        if let Some(browser) = web_sys::window() {
            browser.set_onpopstate(Some(history.as_ref().unchecked_ref()));
        }
        let mut shell = Self {
            host: HostKind::Unknown,
            screen: Screen::Checking,
            page: Page::Loading,
            destination: address(),
            binding: None,
            identifier,
            password,
            message: "",
            logging_out: false,
            focus,
            scroll: gpui_kit::ScrollHandle::new(),
            action_focus: cx.focus_handle().tab_stop(true),
            request: None,
            saves: Vec::new(),
            events,
            changes,
            _changed: changed,
            history,
        };
        shell.check(window, cx);
        shell
    }

    fn signed_out(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.binding = None;
        self.page = Page::Loading;
        self.saves.clear();
        self.events.truncate(2);
        self.logging_out = false;
        self.message = "";
        self.screen = Screen::Identifier;
        self.identifier
            .update(cx, |field, cx| field.set_value("", w, cx));
        self.password
            .update(cx, |field, cx| field.set_value("", w, cx));
        cx.notify();
    }

    fn check(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.page, Page::Board(_)) {
            w.close_dialog(cx);
        }
        self.logging_out = false;
        self.screen = Screen::Checking;
        self.page = Page::Loading;
        self.saves.clear();
        self.events.truncate(2);
        if self.host == HostKind::Viewer {
            self.screen = Screen::SignedIn;
            self.load(w, cx);
            return;
        }
        let executor = cx.background_executor().clone();
        let discover = self.host == HostKind::Unknown;
        self.request = Some(cx.spawn_in(w, async move |view, cx| {
            if discover {
                let host = session::viewer_board(&executor).await;
                let _ = view.update_in(cx, |view, w, cx| match host {
                    Ok(board) => {
                        view.host = if let Some(board) = board {
                            view.binding = Some(DISPOSABLE_VIEWER.to_owned());
                            view.destination.get_or_insert(board);
                            HostKind::Viewer
                        } else {
                            HostKind::Authenticated
                        };
                        view.check(w, cx);
                    }
                    Err(_) => {
                        view.screen = Screen::Unavailable;
                        cx.notify();
                    }
                });
                return;
            }
            let stored = binding::read().await;
            let result = match &stored {
                Ok(Some(binding)) => session::resolve(binding, &executor).await,
                Ok(None) => session::Resolution::Rejected,
                Err(()) => session::Resolution::Unavailable,
            };
            if matches!(result, session::Resolution::Rejected)
                && let Ok(Some(binding)) = &stored
                && binding::forget(binding).await.is_err()
            {
                let _ = view.update_in(cx, |view, _, cx| {
                    view.screen = Screen::StorageUnavailable;
                    cx.notify();
                });
                return;
            }
            let _ = view.update_in(cx, |view, w, cx| {
                match (stored, result) {
                    (Ok(Some(binding)), session::Resolution::Accepted) => {
                        view.binding = Some(binding);
                        view.screen = Screen::SignedIn;
                        view.load(w, cx);
                    }
                    (Err(()), _) => view.screen = Screen::StorageUnavailable,
                    (_, session::Resolution::Unavailable) => view.screen = Screen::Unavailable,
                    _ => view.signed_out(w, cx),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn advance(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.screen, Screen::Identifier) {
            if self.identifier.read(cx).value().is_empty() {
                self.message = "Enter a username or email";
            } else {
                self.screen = Screen::Password;
                self.message = "";
                self.password.update(cx, |p, cx| p.focus(w, cx));
            }
            cx.notify();
            return;
        }
        if !matches!(self.screen, Screen::Password) {
            return;
        }
        let credentials = Credentials {
            identifier: self.identifier.read(cx).value().to_string(),
            password: self.password.read(cx).value().to_string(),
        };
        self.screen = Screen::SigningIn;
        self.message = "";
        let executor = cx.background_executor().clone();
        self.request = Some(cx.spawn_in(w, async move |view, cx| {
            let result = session::sign_in(&credentials, &executor).await;
            drop(credentials);
            let result = match result {
                Ok(binding) => {
                    if binding::save(&binding).await.is_err() {
                        let _ = session::logout(&binding, &executor).await;
                        Err(EditError::Transport)
                    } else {
                        Ok(binding)
                    }
                }
                Err(error) => Err(error),
            };
            let _ = view.update_in(cx, |view, w, cx| {
                view.password.update(cx, |p, cx| p.set_value("", w, cx));
                match result {
                    Ok(_) => view.check(w, cx),
                    Err(error) => {
                        view.screen = Screen::Password;
                        view.message = if error == EditError::SessionRejected {
                            "Incorrect username or password"
                        } else {
                            "Sign-in unavailable. Try again."
                        };
                        view.password.update(cx, |p, cx| p.focus(w, cx));
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn navigate(&mut self, destination: Option<Uuid>, w: &mut Window, cx: &mut Context<Self>) {
        let path = destination.map_or_else(|| "/web/".into(), |id| format!("/web/boards/{id}"));
        let changed = web_sys::window()
            .and_then(|b| b.history().ok())
            .is_some_and(|h| {
                h.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&path))
                    .is_ok()
            });
        if !changed {
            self.message = "Could not open board address";
            cx.notify();
            return;
        }
        self.destination = destination;
        self.check(w, cx);
    }

    fn load(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(binding) = self.binding.clone() else {
            return;
        };
        self.page = Page::Loading;
        self.message = "";
        let destination = self.destination;
        let viewer = self.host == HostKind::Viewer;
        let executor = cx.background_executor().clone();
        // Defer replacing the resolving task until its callback returns.
        self.request = Some(cx.spawn_in(w, async move |view, cx| {
            let result = if let Some(board) = destination {
                session::table(&binding, board, &executor)
                    .await
                    .map(|read| (Vec::new(), Some(read)))
            } else {
                session::list(&binding, &executor)
                    .await
                    .map(|boards| (boards, None))
            };
            if !viewer && matches!(result, Err(EditError::SessionRejected)) {
                let _ = binding::forget(&binding).await;
            }
            let _ = view.update_in(cx, |view, w, cx| {
                match result {
                    Ok((boards, None)) => view.page = Page::List(boards),
                    Ok((_, Some(read))) => {
                        let Some(board) = destination else {
                            return;
                        };
                        let table = cx.new(|cx| {
                            Table::new(binding.clone(), board, read, view.scroll.clone(), w, cx)
                        });
                        view.events.push(cx.subscribe_in(
                            &table,
                            w,
                            |view, _, _: &AccessLost, w, cx| view.check(w, cx),
                        ));
                        view.page = Page::Board(table);
                    }
                    Err(EditError::SessionRejected) if viewer => view.page = Page::Denied,
                    Err(EditError::SessionRejected) => view.signed_out(w, cx),
                    Err(EditError::PermissionDenied | EditError::Missing) => {
                        view.page = Page::Denied
                    }
                    Err(_) => {
                        view.page = if destination.is_some() {
                            Page::BoardUnavailable
                        } else {
                            Page::ListUnavailable
                        }
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn logout(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.logging_out {
            return;
        }
        let Some(binding) = self.binding.clone() else {
            return;
        };
        self.logging_out = true;
        let executor = cx.background_executor().clone();
        self.request = Some(cx.spawn_in(w, async move |view, cx| {
            let acknowledged = session::logout(&binding, &executor).await.is_ok();
            let forgotten = acknowledged && binding::forget(&binding).await.is_ok();
            let _ = view.update_in(cx, |view, w, cx| {
                view.logging_out = false;
                if forgotten {
                    view.signed_out(w, cx);
                } else if acknowledged {
                    view.screen = Screen::StorageUnavailable;
                    view.page = Page::Loading;
                } else {
                    view.message = "Could not log out. Try again.";
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}

fn address() -> Option<Uuid> {
    web_sys::window()?
        .location()
        .pathname()
        .ok()?
        .strip_prefix("/web/boards/")
        .and_then(|id| Uuid::parse_str(id).ok())
}
fn status(id: &'static str, text: &'static str) -> impl IntoElement {
    div().id(id).role(Role::Status).aria_label(text).child(text)
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let checking = matches!(self.screen, Screen::Checking | Screen::SigningIn);
        let mut content = div().flex().flex_col().gap_4();
        content = match self.screen {
            Screen::Identifier => content
                .child(status("session-status", "Not signed in"))
                .child("Username or email")
                .child(Input::new(&self.identifier).aria_label("Username or email"))
                .child(
                    Button::new("next")
                        .label("Next")
                        .on_click(cx.listener(|s, _, w, cx| s.advance(w, cx))),
                ),
            Screen::Password | Screen::SigningIn => content
                .child(status("session-status", "Sign in"))
                .child("Password")
                .child(
                    Input::new(&self.password)
                        .aria_label("Password")
                        .disabled(checking),
                )
                .child(
                    Button::new("sign-in")
                        .label("Sign in")
                        .disabled(checking)
                        .loading(checking)
                        .on_click(cx.listener(|s, _, w, cx| s.advance(w, cx))),
                )
                .child(
                    Button::new("change-identifier")
                        .label("Change username")
                        .disabled(checking)
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.screen = Screen::Identifier;
                            s.password.update(cx, |p, cx| p.set_value("", w, cx));
                            s.message = "";
                            cx.notify();
                        })),
                ),
            Screen::Checking => content.child(status("session-status", "Checking session")),
            Screen::Unavailable => content
                .child(status("session-status", "Could not check session"))
                .child("Your session is kept. Try connecting again."),
            Screen::StorageUnavailable => {
                content.child(status("session-status", "Could not read session storage"))
            }
            Screen::SignedIn => {
                let mut page = if self.host == HostKind::Viewer {
                    content
                } else {
                    content
                        .child(status("session-status", "Signed in"))
                        .child("Your session is ready.")
                };
                page = match &self.page {
                    Page::Loading => page.child(status(
                        "page-status",
                        if self.destination.is_some() {
                            "Loading board…"
                        } else {
                            "Loading boards…"
                        },
                    )),
                    Page::List(boards) => {
                        if boards.is_empty() {
                            page = page.child(status("page-status", "No boards"));
                        }
                        for board in boards {
                            let id = board.id;
                            page = page.child(
                                Button::new(gpui_kit::SharedString::from(format!("board-{id}")))
                                    .label(board.name.clone())
                                    .accessibility_id(id.to_string())
                                    .on_click(
                                        cx.listener(move |s, _, w, cx| s.navigate(Some(id), w, cx)),
                                    ),
                            );
                        }
                        page.child(
                            Button::new("refresh")
                                .label("Refresh")
                                .on_click(cx.listener(|s, _, w, cx| s.check(w, cx))),
                        )
                    }
                    Page::Board(table) => page.child(table.clone()),
                    Page::ListUnavailable => page
                        .child(status("page-status", "Could not load boards"))
                        .child(
                            Button::new("retry-list")
                                .label("Retry board list")
                                .on_click(cx.listener(|s, _, w, cx| s.check(w, cx))),
                        ),
                    Page::Denied | Page::BoardUnavailable => page
                        .child(status(
                            "page-status",
                            if matches!(self.page, Page::Denied) {
                                "Destination unavailable"
                            } else {
                                "Could not load board"
                            },
                        ))
                        .child(
                            Button::new("retry-destination")
                                .label("Retry destination")
                                .on_click(cx.listener(|s, _, w, cx| s.check(w, cx))),
                        ),
                };
                if self.destination.is_some() {
                    page = page.child(
                        Button::new("back")
                            .label("Back to boards")
                            .on_click(cx.listener(|s, _, w, cx| s.navigate(None, w, cx))),
                    );
                }
                page
            }
        };
        let action = match self.screen {
            Screen::Unavailable => "Retry connection",
            Screen::StorageUnavailable => "Retry storage",
            _ => "Check session",
        };
        let mut actions = div().flex().gap_2().child(
            Button::new("check-session")
                .track_focus(&self.action_focus)
                .label(action)
                .loading(checking)
                .disabled(checking)
                .on_click(cx.listener(|s, _, w, cx| s.check(w, cx))),
        );
        if matches!(self.screen, Screen::SignedIn) {
            actions = actions.child(
                Button::new("logout")
                    .label("Log out")
                    .disabled(self.logging_out)
                    .loading(self.logging_out)
                    .on_click(cx.listener(|s, _, w, cx| s.logout(w, cx))),
            );
        }
        div()
            .id("web-shell")
            .role(Role::Application)
            .aria_label("boardcraft")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .p_6()
            .gap_6()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(div().text_lg().child("boardcraft"))
            .child(
                div()
                    .id("content-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(content),
            )
            .child(status("notice", self.message))
            .children((self.host != HostKind::Viewer).then_some(actions))
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        if let Some(changes) = &self.changes {
            changes.set_onmessage(None);
            changes.close();
        }
        if let Some(browser) = web_sys::window() {
            browser.set_onpopstate(None);
        }
        let _ = &self.history;
    }
}
