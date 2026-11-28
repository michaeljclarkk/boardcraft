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
    navigation::{Navigation, NavigationEvent},
    session,
};

mod workspace;

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
    List,
    Board(Entity<Table>),
    Denied,
    BoardUnavailable,
}

pub(super) struct Shell {
    host: HostKind,
    screen: Screen,
    page: Page,
    boards: Option<Vec<BoardSummary>>,
    destination: Option<Uuid>,
    binding: Option<String>,
    identifier: Entity<InputState>,
    password: Entity<InputState>,
    navigation: Entity<Navigation>,
    message: &'static str,
    logging_out: bool,
    focus: FocusHandle,
    scroll: gpui_kit::ScrollHandle,
    action_focus: FocusHandle,
    request: Option<Task<()>>,
    saves: Vec<Task<()>>,
    events: Vec<Subscription>,
    _navigation_events: Subscription,
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
        let navigation = cx.new(|cx| Navigation::new(window, cx));
        let navigation_events =
            cx.subscribe_in(&navigation, window, |shell, _, event, w, cx| match event {
                NavigationEvent::Open(id) => shell.navigate(*id, w, cx),
                NavigationEvent::Refresh => shell.check(w, cx),
                NavigationEvent::LogOut => shell.logout(w, cx),
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
                    shell
                        .navigation
                        .update(cx, |navigation, cx| navigation.dismiss(w, cx));
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
            boards: None,
            destination: address(),
            binding: None,
            identifier,
            password,
            navigation,
            message: "",
            logging_out: false,
            focus,
            scroll: gpui_kit::ScrollHandle::new(),
            action_focus: cx.focus_handle().tab_stop(true),
            request: None,
            saves: Vec::new(),
            events,
            _navigation_events: navigation_events,
            changes,
            _changed: changed,
            history,
        };
        shell.check(window, cx);
        shell
    }

    fn signed_out(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.navigation = cx.new(|cx| Navigation::new(w, cx));
        self._navigation_events =
            cx.subscribe_in(&self.navigation, w, |shell, _, event, w, cx| match event {
                NavigationEvent::Open(id) => shell.navigate(*id, w, cx),
                NavigationEvent::Refresh => shell.check(w, cx),
                NavigationEvent::LogOut => shell.logout(w, cx),
            });
        self.binding = None;
        self.page = Page::Loading;
        self.boards = None;
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
        self.boards = None;
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
                        view.host = if board.is_some() {
                            view.binding = Some(DISPOSABLE_VIEWER.to_owned());
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
        if self.logging_out || destination == self.destination {
            return;
        }
        self.navigation
            .update(cx, |navigation, cx| navigation.dismiss(w, cx));
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
        if matches!(self.page, Page::Board(_)) {
            w.close_dialog(cx);
        }
        self.saves.clear();
        self.events.truncate(2);
        self.scroll = gpui_kit::ScrollHandle::new();
        self.load(w, cx);
    }

    fn load(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(binding) = self.binding.clone() else {
            return;
        };
        self.page = Page::Loading;
        self.message = "";
        self.navigation.update(cx, |navigation, cx| {
            navigation.loading(self.host == HostKind::Authenticated, cx)
        });
        let destination = self.destination;
        let viewer = self.host == HostKind::Viewer;
        let executor = cx.background_executor().clone();
        // Defer replacing the resolving task until its callback returns.
        self.request = Some(cx.spawn_in(w, async move |view, cx| {
            // Listing is bounded. Even a board outside that list can be opened by
            // address: the host checks access again on the table read.
            let (boards, detail) = futures::join!(session::list(&binding, &executor), async {
                match destination {
                    Some(id) => Some((id, session::table(&binding, id, &executor).await)),
                    None => None,
                }
            });
            if matches!(boards, Err(EditError::SessionRejected))
                || matches!(detail, Some((_, Err(EditError::SessionRejected))))
            {
                if !viewer {
                    let _ = binding::forget(&binding).await;
                }
                let _ = view.update_in(cx, |view, w, cx| {
                    if viewer {
                        view.boards = None;
                        view.page = Page::Denied;
                        view.navigation.update(cx, |navigation, cx| {
                            navigation.loaded(None, view.destination, true, w, cx)
                        });
                        cx.notify();
                    } else {
                        view.signed_out(w, cx);
                    }
                });
                return;
            }
            let _ = view.update_in(cx, |view, w, cx| {
                let list_denied = matches!(
                    boards,
                    Err(EditError::PermissionDenied | EditError::Missing)
                );
                view.boards = boards.ok();
                view.page = match detail {
                    None if list_denied => Page::Denied,
                    None => Page::List,
                    Some((board, Ok(read))) => {
                        let table = cx.new(|cx| {
                            Table::new(binding.clone(), board, read, view.scroll.clone(), w, cx)
                        });
                        view.events.push(cx.subscribe_in(
                            &table,
                            w,
                            |view, _, _: &AccessLost, w, cx| view.check(w, cx),
                        ));
                        Page::Board(table)
                    }
                    Some((id, Err(EditError::PermissionDenied | EditError::Missing))) => {
                        if let Some(boards) = &mut view.boards {
                            boards.retain(|board| board.id != id);
                        }
                        Page::Denied
                    }
                    Some((_, Err(_))) => Page::BoardUnavailable,
                };
                view.navigation.update(cx, |navigation, cx| {
                    navigation.loaded(
                        view.boards.as_deref(),
                        view.destination,
                        matches!(view.page, Page::Denied),
                        w,
                        cx,
                    )
                });
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
        self.navigation
            .update(cx, |navigation, cx| navigation.set_busy(true, cx));
        let executor = cx.background_executor().clone();
        self.request = Some(cx.spawn_in(w, async move |view, cx| {
            let acknowledged = session::logout(&binding, &executor).await.is_ok();
            let forgotten = acknowledged && binding::forget(&binding).await.is_ok();
            let _ = view.update_in(cx, |view, w, cx| {
                view.logging_out = false;
                view.navigation
                    .update(cx, |navigation, cx| navigation.set_busy(false, cx));
                if forgotten {
                    view.signed_out(w, cx);
                } else if acknowledged {
                    view.screen = Screen::StorageUnavailable;
                    view.boards = None;
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
            Screen::SignedIn => return self.workspace(cx).into_any_element(),
        };
        let action = match self.screen {
            Screen::Unavailable => "Retry connection",
            Screen::StorageUnavailable => "Retry storage",
            _ => "Check session",
        };
        let actions = div().flex().gap_2().child(
            Button::new("check-session")
                .track_focus(&self.action_focus)
                .label(action)
                .loading(checking)
                .disabled(checking)
                .on_click(cx.listener(|s, _, w, cx| s.check(w, cx))),
        );
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
            .into_any_element()
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
