use super::*;
use boardcraft_mobile_edit_client::Credentials;
use session::{Open, SessionError};

pub(super) enum Login {
    Identifier(Entity<InputState>),
    Password {
        identifier: String,
        input: Entity<InputState>,
        pending: bool,
    },
    Loading,
    RetryLoad,
    RetryStorage,
    RetryForget,
    Denied,
}

impl Login {
    pub(super) fn identifier(window: &mut Window, cx: &mut Context<BoardView>) -> Self {
        Self::Identifier(cx.new(|cx| InputState::new(window, cx).placeholder("Username or email")))
    }

    pub(super) fn form(
        &self,
        mut column: Div,
        message: &'static str,
        cx: &mut Context<BoardView>,
    ) -> Div {
        let (button, pending) = match self {
            Self::Identifier(input) => {
                column = column.child(Input::new(input).large());
                ("Next", false)
            }
            Self::Password { input, pending, .. } => {
                column = column.child(Input::new(input).large().disabled(*pending));
                ("Sign in", *pending)
            }
            Self::Loading => return column.child(Label::new("Loading…")),
            Self::RetryLoad => ("Retry connection", false),
            Self::RetryStorage => ("Retry storage", false),
            Self::RetryForget => ("Retry cleanup", false),
            Self::Denied => ("Log out", false),
        };
        column = column.child(
            Button::new("continue")
                .large()
                .label(button)
                .disabled(pending)
                .on_click(cx.listener(|view, _, window, cx| view.advance_login(window, cx))),
        );
        if !message.is_empty() {
            column = column.child(Label::new(message));
        }
        column
    }
}

impl BoardView {
    fn advance_login(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn) = self.connection else { return };
        self.message = "";
        match &mut self.login {
            Some(Login::Identifier(input)) => {
                let identifier = input.read(cx).value().to_string();
                if identifier.is_empty() {
                    self.message = "Enter a username or email";
                } else {
                    self.login = Some(Login::Password {
                        identifier,
                        input: cx.new(|cx| {
                            InputState::new(window, cx)
                                .placeholder("Password")
                                .masked(true)
                        }),
                        pending: false,
                    });
                }
            }
            Some(Login::Password {
                identifier,
                input,
                pending,
            }) if !*pending => {
                *pending = true;
                let credentials = Credentials {
                    identifier: identifier.clone(),
                    password: input.read(cx).value().to_string(),
                };
                let signed_out = Login::identifier(window, cx);
                let work = cx.background_executor().spawn(async move {
                    EditClient::sign_in_with_storage(conn.port, credentials, |s| {
                        session::save(conn.port, s)
                    })
                });
                cx.spawn(async move |view, cx| {
                    let result = work.await;
                    _ = view.update(cx, |view, cx| {
                        match result {
                            Ok(client) => {
                                view.load_board(conn, Some(client), signed_out, cx);
                            }
                            Err(error) => {
                                if let Some(Login::Password { pending, .. }) = &mut view.login {
                                    *pending = false;
                                }
                                view.message = match error {
                                    SessionError::Remote(EditError::SessionRejected) => {
                                        "Incorrect username or password"
                                    }
                                    SessionError::Remote(_) => "Sign-in unavailable. Try again.",
                                    SessionError::Local => "Could not store session. Try again.",
                                };
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            Some(Login::RetryLoad | Login::RetryStorage) => self.restore(conn, window, cx),
            Some(Login::RetryForget) => {
                let identifier = Login::identifier(window, cx);
                self.forget_session(conn, identifier, cx);
            }
            Some(Login::Denied) => self.logout(window, cx),
            _ => return,
        }
        cx.notify();
    }

    pub(super) fn restore(
        &mut self,
        conn: Connection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let identifier = Login::identifier(window, cx);
        self.load_board(conn, None, identifier, cx);
    }

    fn load_board(
        &mut self,
        conn: Connection,
        issued: Option<EditClient>,
        identifier: Login,
        cx: &mut Context<Self>,
    ) {
        self.login = Some(Login::Loading);
        let work = cx
            .background_executor()
            .spawn(async move { session::open(conn, issued) });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            _ = view.update(cx, |view, cx| {
                match result {
                    Open::Missing => view.login = Some(identifier),
                    Open::Unreadable => {
                        view.login = Some(Login::RetryStorage);
                        view.message = "Could not read stored session.";
                    }
                    Open::Board(client, rows) => {
                        view.client = Some(client);
                        view.board.items = rows.into_iter().map(Item::from).collect();
                        view.login = None;
                        view.message = "";
                    }
                    Open::Browse(client, list) => {
                        view.client = Some(client);
                        view.navigation = Some(navigation::Page::List(list));
                        view.login = None;
                        view.message = "";
                    }
                    Open::Rejected => {
                        view.forget_session(conn, identifier, cx);
                    }
                    Open::Denied(client) => {
                        view.client = Some(client);
                        view.login = Some(Login::Denied);
                        view.message = "No access to this board.";
                    }
                    Open::Unavailable => {
                        view.login = Some(Login::RetryLoad);
                        view.message = "";
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn forget_session(
        &mut self,
        conn: Connection,
        identifier: Login,
        cx: &mut Context<Self>,
    ) {
        self.client = None;
        self.navigation = None;
        self.editor = None;
        self.board.items.clear();
        self.message = "";
        self.login = Some(Login::Loading);
        let work = cx
            .background_executor()
            .spawn(async move { session::forget(conn.port) });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            _ = view.update(cx, |view, cx| {
                view.login = Some(if result.is_ok() {
                    identifier
                } else {
                    view.message = "Could not remove stored session.";
                    Login::RetryForget
                });
                cx.notify();
            });
        })
        .detach();
    }
}
