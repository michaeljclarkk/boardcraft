//! Read-only HTTP entry. Only the host has a database connection or makes access decisions.
use boardcraft_mobile_edit_client::{Credentials, EditClient, EditError, SavedItem};
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    button::Button,
    input::{Input, InputState},
    label::Label,
};
use gpui_kit::*;
use uuid::Uuid;

enum Screen {
    Identifier(Entity<InputState>),
    Password(String, Entity<InputState>),
    Loading,
    Retry(EditClient),
    Board {
        _session: EditClient,
        rows: Vec<SavedItem>,
    },
}

struct Connected {
    port: u16,
    board: Uuid,
    screen: Screen,
    message: &'static str,
}

pub(super) fn open(port: u16, board: Uuid, cx: &mut App) {
    let (_window, _) = open_window(WindowOptions::default(), cx, |window, cx| {
        cx.new(|cx| Connected {
            port,
            board,
            screen: Connected::identifier(window, cx),
            message: "",
        })
    })
    .expect("open connected board window");
    cx.activate(true);
    #[cfg(all(windows, feature = "native-smoke"))]
    if std::env::args().nth(4).as_deref() == Some("--smoke-login") {
        cx.update_window(_window, |_, w, _| {
            super::native_smoke::login::install(board, w)
        })
        .unwrap();
    }
}

impl Connected {
    fn identifier(w: &mut Window, cx: &mut Context<Self>) -> Screen {
        Screen::Identifier(cx.new(|cx| InputState::new(w, cx).placeholder("Username or email")))
    }

    fn password(identifier: String, w: &mut Window, cx: &mut Context<Self>) -> Screen {
        Screen::Password(
            identifier,
            cx.new(|cx| InputState::new(w, cx).placeholder("Password").masked(true)),
        )
    }

    fn advance(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.message = "";
        match &self.screen {
            Screen::Identifier(field) => {
                let identifier = field.read(cx).value().to_string();
                if identifier.is_empty() {
                    self.message = "Enter a username or email";
                } else {
                    self.screen = Self::password(identifier, w, cx);
                }
            }
            Screen::Password(identifier, field) => {
                let identifier = identifier.clone();
                let credentials = Credentials {
                    identifier: identifier.clone(),
                    password: field.read(cx).value().to_string(),
                };
                let port = self.port;
                self.screen = Screen::Loading;
                let work = cx
                    .background_executor()
                    .spawn(async move { EditClient::sign_in(port, credentials) });
                cx.spawn_in(w, async move |view, cx| {
                    let result = work.await;
                    _ = view.update_in(cx, |view, w, cx| match result {
                        Ok(client) => view.load(client, w, cx),
                        Err(error) => {
                            view.screen = Self::password(identifier, w, cx);
                            view.message = if error == EditError::SessionRejected {
                                "Incorrect username or password"
                            } else {
                                "Sign-in unavailable. Try again."
                            };
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
            Screen::Retry(client) => self.load(client.clone(), w, cx),
            _ => return,
        }
        cx.notify();
    }

    fn load(&mut self, client: EditClient, w: &mut Window, cx: &mut Context<Self>) {
        self.screen = Screen::Loading;
        let board = self.board;
        let work = cx.background_executor().spawn(async move {
            let rows = client.resolve().and_then(|()| client.read(board));
            (client, rows)
        });
        cx.spawn_in(w, async move |view, cx| {
            let (client, rows) = work.await;
            _ = view.update_in(cx, |view, w, cx| {
                view.screen = match rows {
                    Ok(rows) => Screen::Board {
                        _session: client,
                        rows,
                    },
                    Err(EditError::SessionRejected) => {
                        view.message = "Session expired. Sign in again.";
                        Self::identifier(w, cx)
                    }
                    Err(e) => {
                        view.message = match e {
                            EditError::PermissionDenied => "No access to this board.",
                            EditError::Missing => "Board not found.",
                            _ => "Could not load board.",
                        };
                        Screen::Retry(client)
                    }
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for Connected {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut column = div()
            .id("connected")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .p_4()
            .gap_3();
        let action = match &self.screen {
            Screen::Identifier(field) => {
                column = column.child(Input::new(field).id("identifier"));
                "Next"
            }
            Screen::Password(_, field) => {
                column = column.child(Input::new(field).id("password"));
                "Sign in"
            }
            Screen::Loading => return column.child(Label::new("Loading…")),
            Screen::Retry(_) => "Retry loading",
            Screen::Board { rows, .. } => {
                column = column.child(Label::new("Board items · Read only"));
                if rows.is_empty() {
                    return column.child(Label::new("Empty board"));
                }
                return column.children(rows.iter().map(|row| {
                    Button::new(SharedString::from(format!("title-{}", row.id)))
                        .disabled(true)
                        .label(row.title.clone())
                }));
            }
        };
        column = column.child(
            Button::new("continue")
                .label(action)
                .on_click(cx.listener(|view, _, w, cx| view.advance(w, cx))),
        );
        if matches!(self.screen, Screen::Password(..)) {
            column = column.child(Button::new("back").label("Back").on_click(cx.listener(
                |view, _, w, cx| {
                    if matches!(view.screen, Screen::Password(..)) {
                        view.screen = Self::identifier(w, cx);
                        view.message = "";
                        cx.notify();
                    }
                },
            )));
        }
        column.child(Label::new(self.message))
    }
}
