use super::*;
use boardcraft_mobile_edit_client::{BoardSummary, SavedItem};

enum Page {
    List(Vec<BoardSummary>),
    Items(Vec<SavedItem>),
    Unavailable,
    Fault,
}

pub(super) struct Navigation {
    client: EditClient,
    page: Page,
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
        self.navigation_request += 1;
        let request = self.navigation_request;
        let work = cx.background_executor().spawn(async move {
            let result = match board {
                Some(id) => client.read_selected_board(id).map(Page::Items),
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
                    view.screen = Screen::Navigation(Navigation { client, page });
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
            Page::Items(rows) => {
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
