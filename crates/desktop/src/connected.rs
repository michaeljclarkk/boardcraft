//! HTTP entry. Only the host has a database connection or makes access decisions.
use super::{Row, RowRecovery};
use boardcraft_board::{BoardId, ItemId, store::StoredItem};
use boardcraft_desktop::Save;
use boardcraft_mobile_edit_client::{Credentials, EditClient, EditError};
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    button::Button,
    input::{Input, InputState},
    label::Label,
};
use gpui_kit::*;
use uuid::Uuid;
mod board_link;
mod main_table;
mod navigation;
use navigation::Navigation;

enum Screen {
    Identifier(Entity<InputState>),
    Password(String, Entity<InputState>),
    Loading,
    Retry(EditClient),
    Navigation(Navigation),
    Board {
        session: EditClient,
        logging_out: bool,
        can_edit: bool,
        rows: Vec<Entity<Row>>,
    },
}

pub(super) struct Connected {
    port: u16,
    direct_board: Option<Uuid>,
    navigation_request: u64,
    link_input: Option<Entity<InputState>>,
    screen: Screen,
    message: &'static str,
    scroll: ScrollHandle,
    #[cfg(all(windows, feature = "native-smoke"))]
    pub(super) submissions: usize,
    #[cfg(all(windows, feature = "native-smoke"))]
    pub(super) loads: usize,
}

/// Decide locally, before constructing any request. Never interpret the supplied origin as a URL.
#[cfg(windows)]
fn configured_session(port: u16) -> Result<Option<EditClient>, ()> {
    let origin = std::env::var_os("BOARDCRAFT_SESSION_ORIGIN");
    let token = std::env::var_os("BOARDCRAFT_SESSION_TOKEN");
    match (origin, token) {
        (None, None) => Ok(None),
        (Some(origin), Some(token))
            if origin.to_str() == Some(&format!("http://127.0.0.1:{port}")) =>
        {
            EditClient::with_bearer(port, token.to_str().ok_or(())?)
                .map(Some)
                .map_err(|_| ())
        }
        _ => Err(()),
    }
}

pub(super) fn open(port: u16, board: Option<Uuid>, cx: &mut App) {
    let (_window, _view) = open_window(WindowOptions::default(), cx, |window, cx| {
        cx.new(|cx| {
            let mut view = Connected::new(port, board);
            #[cfg(windows)]
            match configured_session(port) {
                Ok(Some(client)) => {
                    view.load(client, "", window, cx);
                    return view;
                }
                Ok(None) => (),
                Err(()) => view.message = "Session configuration ignored. Sign in to continue.",
            }
            view.screen = Connected::identifier(window, cx);
            view
        })
    })
    .expect("open connected board window");
    cx.activate(true);
    #[cfg(all(windows, feature = "native-smoke"))]
    if let Some(mode) = std::env::args().nth(if board.is_some() { 4 } else { 3 }) {
        cx.update_window(_window, |_, w, cx| {
            if mode.starts_with("--name-") {
                super::native_smoke::name::install(_view, &mode, w);
            } else if mode.starts_with("--logout-") {
                super::native_smoke::logout::install(_view, &mode, w);
            } else if mode.starts_with("--link-") {
                super::native_smoke::board_link::install(&mode, port, w);
            } else if mode.starts_with("--table-") {
                super::native_smoke::main_table::install(_view, &mode, w);
            } else if mode.starts_with("--entry-") {
                super::native_smoke::grouped_entry::install(_view, &mode, w);
            } else if mode.starts_with("--bulk-archive-") {
                cx.set_reduce_motion(true);
                super::native_smoke::selected_archive::install(_view, &mode, w);
            } else if mode.starts_with("--selection-") {
                cx.set_reduce_motion(true);
                super::native_smoke::selection_move::install(_view, &mode, w);
            } else if mode.starts_with("--archive-") {
                cx.set_reduce_motion(true);
                super::native_smoke::archive::install(_view, &mode, w);
            } else if mode.starts_with("--groups-") {
                cx.set_reduce_motion(true);
                super::native_smoke::group_fields::install(_view, &mode, w);
            } else if mode.starts_with("--order-") {
                cx.set_reduce_motion(true);
                super::native_smoke::group_order::install(_view, &mode, w);
            } else if mode.starts_with("--placement-") {
                cx.set_reduce_motion(true);
                super::native_smoke::item_placement::install(_view, &mode, w);
            } else if let Some(board) = board {
                super::native_smoke::login::install(board, mode == "--smoke-session", w);
            } else {
                super::native_smoke::navigation::install(mode == "--browse-revoked", w);
            }
        })
        .unwrap();
    }
}

impl Connected {
    fn new(port: u16, direct_board: Option<Uuid>) -> Self {
        Self {
            port,
            direct_board,
            navigation_request: 0,
            link_input: None,
            screen: Screen::Loading,
            message: "",
            scroll: ScrollHandle::new(),
            #[cfg(all(windows, feature = "native-smoke"))]
            submissions: 0,
            #[cfg(all(windows, feature = "native-smoke"))]
            loads: 0,
        }
    }
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
                        Ok(client) => view.load(client, "", w, cx),
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
            Screen::Retry(client) => self.load(client.clone(), "", w, cx),
            _ => return,
        }
        cx.notify();
    }

    fn load(
        &mut self,
        client: EditClient,
        notice: &'static str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(board) = self.direct_board else {
            self.navigate(client, None, w, cx);
            return;
        };
        self.screen = Screen::Loading;
        #[cfg(all(windows, feature = "native-smoke"))]
        {
            self.loads += 1;
        }
        let work = cx.background_executor().spawn(async move {
            let rows = client
                .resolve()
                .and_then(|()| client.read_with_access(board))
                .and_then(|read| {
                    let rows = read
                        .rows
                        .into_iter()
                        .map(|r| {
                            if r.revision <= 0 {
                                return Err(EditError::Transport);
                            }
                            Ok(StoredItem {
                                id: ItemId::new(r.id).ok_or(EditError::Transport)?,
                                title: r.title,
                                revision: r.revision,
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok((rows, read.can_edit))
                });
            (client, rows)
        });
        cx.spawn_in(w, async move |view, cx| {
            let (client, rows) = work.await;
            _ = view.update_in(cx, |view, w, cx| {
                view.screen = match rows {
                    Ok((items, can_edit)) => {
                        view.message = notice;
                        let board = BoardId::new(board).expect("validated at launch");
                        let rows = items
                            .into_iter()
                            .map(|item| {
                                let row = cx.new(|cx| {
                                    Row::new(can_edit, board, item, RowRecovery::default(), w, cx)
                                });
                                if can_edit {
                                    cx.subscribe_in(&row, w, |view, row, save: &Save, w, cx| {
                                        view.save(row.clone(), save.clone(), w, cx)
                                    })
                                    .detach();
                                }
                                row
                            })
                            .collect();
                        Screen::Board {
                            session: client,
                            logging_out: false,
                            can_edit,
                            rows,
                        }
                    }
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

    fn logout(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Board {
            session,
            rows,
            logging_out,
            ..
        } = &mut self.screen
        else {
            return;
        };
        // A changed blur/Enter finishes through the normal save path first.
        // Loads have no board controls; saves and logout never overlap.
        if *logging_out
            || rows.iter().any(|r| {
                let row = r.read(cx);
                row.baseline.is_some() || row.edit.phase() == &boardcraft_desktop::Phase::Pending
            })
        {
            return;
        }
        *logging_out = true;
        self.message = "";
        let client = session.clone();
        let work = cx
            .background_executor()
            .spawn(async move { client.logout() });
        cx.spawn_in(w, async move |view, cx| {
            let result = work.await;
            _ = view.update_in(cx, |view, w, cx| {
                if result.is_ok() {
                    view.screen = Self::identifier(w, cx);
                    view.message = "";
                } else if let Screen::Board { logging_out, .. } = &mut view.screen {
                    *logging_out = false;
                    view.message = "Could not log out";
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn save(&mut self, row: Entity<Row>, save: Save, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Board {
            session,
            can_edit: true,
            logging_out: false,
            rows,
        } = &self.screen
        else {
            return;
        };
        if !rows.contains(&row) {
            return;
        }
        let client = session.clone();
        self.message = "";
        #[cfg(all(windows, feature = "native-smoke"))]
        {
            self.submissions += 1;
        }
        let work = cx
            .background_executor()
            .spawn(async move { save.send(&client) });
        cx.spawn_in(w, async move |view, cx| {
            let result = work.await;
            _ = view.update_in(cx, |view, w, cx| {
                let Screen::Board { session, rows, .. } = &view.screen else {
                    return;
                };
                if !rows.contains(&row) {
                    return;
                }
                match result {
                    Ok(item) => row.update(cx, |row, cx| {
                        row.edit.finish(Ok(item));
                        cx.notify();
                    }),
                    Err(EditError::SessionRejected) => {
                        view.screen = Self::identifier(w, cx);
                        view.message = "Session expired. Sign in again.";
                    }
                    Err(error) => {
                        let notice = match error {
                            EditError::PermissionDenied => "Not saved: permission denied.",
                            EditError::InvalidDraft => {
                                "Not saved: enter a nonblank name of at most 255 UTF-16 units."
                            }
                            EditError::Conflict => "Not saved: name revision conflict.",
                            EditError::Missing => "Not saved: item or board no longer exists.",
                            EditError::AccessUnavailable => "Not saved: access check unavailable.",
                            _ => "Save not confirmed. Check the reloaded name.",
                        };
                        // Discard this board and its pending projection. Only a fresh
                        // authorized GET may show rows again; never replay the PUT.
                        view.load(session.clone(), notice, w, cx);
                    }
                }
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
            .track_scroll(&self.scroll)
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
            Screen::Navigation(nav) => {
                return column
                    .child(nav.render(cx))
                    .child(
                        Input::new(self.link_input.as_ref().expect("browse input"))
                            .id("board-link"),
                    )
                    .child(
                        Button::new("open-link")
                            .label("Open board link")
                            .on_click(cx.listener(|view, _, w, cx| view.open_link(false, w, cx))),
                    )
                    .child(Label::new(self.message));
            }
            Screen::Retry(_) => "Retry loading",
            Screen::Board {
                rows,
                can_edit,
                logging_out,
                ..
            } => {
                column = column
                    .child(Label::new(if *can_edit {
                        "Board items"
                    } else {
                        "Board items · Read only"
                    }))
                    .child(Label::new(self.message))
                    .child(
                        Button::new("logout")
                            .label("Log out")
                            .disabled(*logging_out)
                            .on_click(cx.listener(|view, _, w, cx| view.logout(w, cx))),
                    );
                if *logging_out {
                    return column.child(Label::new("Logging out…"));
                }
                if rows.is_empty() {
                    return column.child(Label::new("Empty board"));
                }
                return column.children(rows.iter().cloned());
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
