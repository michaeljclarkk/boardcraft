use super::*;
use boardcraft_mobile_edit_client::{BoardSummary, SavedItem};

enum Page {
    List(Vec<BoardSummary>),
    Items(Uuid, Vec<SavedItem>),
    Unavailable,
    Fault,
}

pub(super) struct Navigation {
    client: EditClient,
    page: Page,
    share: Option<Entity<InputState>>,
}

impl Connected {
    pub(super) fn navigate(
        &mut self,
        client: EditClient,
        board: Option<Uuid>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.screen = Screen::Loading;
        self.message = "";
        self.navigation_request += 1;
        let request = self.navigation_request;
        let work = cx.background_executor().spawn(async move {
            let result = match board {
                Some(id) => client
                    .read_selected_board(id)
                    .map(|rows| Page::Items(id, rows)),
                None => client
                    .resolve()
                    .and_then(|()| client.list_boards())
                    .map(Page::List),
            };
            (client, result)
        });
        cx.spawn_in(w, async move |view, cx| {
            let (client, result) = work.await;
            _ = view.update_in(cx, |view, w, cx| {
                if request != view.navigation_request || !matches!(view.screen, Screen::Loading) {
                    return;
                }
                if matches!(result, Err(EditError::SessionRejected)) {
                    view.link_input = None;
                    view.message = "Session expired. Sign in again.";
                    view.screen = Self::identifier(w, cx);
                } else {
                    let page = match result {
                        Ok(page) => page,
                        Err(EditError::PermissionDenied | EditError::Missing)
                            if board.is_some() =>
                        {
                            Page::Unavailable
                        }
                        Err(_) => Page::Fault,
                    };
                    view.link_input.get_or_insert_with(|| {
                        cx.new(|cx| InputState::new(w, cx).placeholder("Paste board address"))
                    });
                    view.screen = Screen::Navigation(Navigation {
                        client,
                        page,
                        share: None,
                    });
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn follow(&mut self, board: Option<Uuid>, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        if let Some(id) = board
            && !matches!(&nav.page, Page::List(list) if list.iter().any(|s| s.id == id))
        {
            return;
        }
        self.navigate(nav.client.clone(), board, w, cx);
    }

    fn share(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(Navigation {
            page: Page::Items(id, _),
            share,
            ..
        }) = &mut self.screen
        else {
            return;
        };
        *share =
            Some(cx.new(|cx| {
                InputState::new(w, cx).default_value(board_link::address(self.port, *id))
            }));
        self.message = "";
        cx.notify();
    }

    fn copy_link(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(Navigation {
            share: Some(field), ..
        }) = &self.screen
        else {
            return;
        };
        #[cfg(windows)]
        let copied = board_link::copy(field.read(cx).value().as_ref()).is_ok();
        #[cfg(not(windows))]
        let copied = false;
        self.message = if copied {
            "Link copied"
        } else {
            field.update(cx, |input, cx| {
                input.focus(w, cx);
                input.select_all(w, cx);
            });
            "Could not copy. Press Ctrl+C to copy the selected address."
        };
        cx.notify();
    }

    pub(super) fn open_link(&mut self, new_window: bool, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        let Some(field) = (if new_window {
            &nav.share
        } else {
            &self.link_input
        }) else {
            return;
        };
        let port = self.port;
        let Some(id) = board_link::parse(port, field.read(cx).value().as_ref()) else {
            self.message = "Enter a board link from this connection.";
            field.update(cx, |input, cx| input.focus(w, cx));
            cx.notify();
            return;
        };
        let client = nav.client.clone();
        if new_window {
            let opened = open_window(WindowOptions::default(), cx, move |w, cx| {
                cx.new(|cx| {
                    let mut view = Self::new(port, None);
                    view.navigate(client, Some(id), w, cx);
                    view
                })
            });
            if opened.is_err() {
                self.message = "Could not open window. Try again.";
                cx.notify();
            }
        } else {
            self.navigate(client, Some(id), w, cx);
        }
    }
}

impl Navigation {
    pub(super) fn render(&self, cx: &mut Context<Connected>) -> impl IntoElement {
        let mut column = div().flex().flex_col().gap_3();
        let action = match &self.page {
            Page::List(list) => {
                column = column.child(Label::new(if list.is_empty() {
                    "No boards"
                } else {
                    "Boards"
                }));
                for summary in list {
                    let id = summary.id;
                    column = column.child(
                        Button::new(SharedString::from(format!("board-{id}")))
                            .label(summary.name.clone())
                            .on_click(
                                cx.listener(move |view, _, w, cx| view.follow(Some(id), w, cx)),
                            ),
                    );
                }
                "Refresh"
            }
            Page::Items(_, rows) => {
                column = column.child(Label::new("Board items · Read only"));
                if rows.is_empty() {
                    column = column.child(Label::new("Empty board"));
                }
                for row in rows {
                    column = column.child(
                        Button::new(SharedString::from(format!("title-{}", row.id)))
                            .label(row.title.clone())
                            .disabled(true),
                    );
                }
                column = column.child(
                    Button::new("share")
                        .label("Share")
                        .on_click(cx.listener(|view, _, w, cx| view.share(w, cx))),
                );
                if let Some(address) = &self.share {
                    column = column
                        .child(Label::new("Board"))
                        .child(Input::new(address).id("share-address").readonly(true))
                        .child(Label::new("Recipients need existing access to this board."))
                        .child(
                            Button::new("copy-link")
                                .label("Copy link")
                                .on_click(cx.listener(|view, _, w, cx| view.copy_link(w, cx))),
                        )
                        .child(
                            Button::new("new-window")
                                .label("Open in new window")
                                .on_click(
                                    cx.listener(|view, _, w, cx| view.open_link(true, w, cx)),
                                ),
                        );
                }
                "Back to boards"
            }
            Page::Unavailable => {
                column = column.child(Label::new("Destination unavailable"));
                "Back to boards"
            }
            Page::Fault => {
                column = column.child(Label::new("Could not load navigation."));
                "Retry board list"
            }
        };
        column.child(
            Button::new("board-list")
                .label(action)
                .on_click(cx.listener(|view, _, w, cx| view.follow(None, w, cx))),
        )
    }
}
