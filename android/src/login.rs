use super::*;
use boardcraft_mobile_edit_client::Credentials;

pub(super) enum Login {
    Identifier(Entity<InputState>),
    Password {
        identifier: String,
        input: Entity<InputState>,
        pending: bool,
    },
    Loading,
    RetryLoad,
    Expired,
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
            Self::RetryLoad => ("Retry", false),
            Self::Expired => ("Sign in", false),
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
            Some(Login::Expired) => self.login = Some(Login::identifier(window, cx)),
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
                let work = cx
                    .background_executor()
                    .spawn(async move { EditClient::sign_in(conn.port, credentials) });
                cx.spawn(async move |view, cx| {
                    let result = work.await;
                    _ = view.update(cx, |view, cx| {
                        match result {
                            Ok(client) => {
                                view.client = Some(client);
                                view.load_after_login(cx);
                            }
                            Err(error) => {
                                if let Some(Login::Password { pending, .. }) = &mut view.login {
                                    *pending = false;
                                }
                                view.message = if error == EditError::SessionRejected {
                                    "Incorrect username or password"
                                } else {
                                    "Sign-in unavailable. Try again."
                                };
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            Some(Login::RetryLoad) => self.load_after_login(cx),
            _ => return,
        }
        cx.notify();
    }

    fn load_after_login(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(conn)) = (self.client.clone(), self.connection) else {
            return;
        };
        self.login = Some(Login::Loading);
        let work = cx.background_executor().spawn(async move {
            client.resolve()?;
            client.read(conn.board)
        });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            _ = view.update(cx, |view, cx| {
                match result {
                    Ok(rows) => {
                        view.board.items = rows.into_iter().map(Item::from).collect();
                        view.login = None;
                        view.message = "";
                    }
                    Err(EditError::SessionRejected) => {
                        view.client = None;
                        view.login = Some(Login::Expired);
                        view.message = "Session expired";
                    }
                    Err(EditError::PermissionDenied) => {
                        view.client = None;
                        view.login = Some(Login::Expired);
                        view.message = "No access to this board.";
                    }
                    Err(_) => {
                        view.login = Some(Login::RetryLoad);
                        view.message = "Could not load board.";
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
