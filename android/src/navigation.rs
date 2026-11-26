use super::*;
use boardcraft_mobile_edit_client::BoardSummary;

pub(super) enum Page {
    List(Vec<BoardSummary>),
    Loading,
    Selected(Vec<SavedItem>),
    Denied,
    Fault,
}

impl BoardView {
    fn navigate(&mut self, board: Option<Uuid>, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(client), Some(page)) =
            (self.connection, self.client.clone(), &self.navigation)
        else {
            return;
        };
        if self.logging_out
            || matches!(page, Page::Loading)
            || board.is_some_and(
                |id| !matches!(page, Page::List(boards) if boards.iter().any(|b| b.id == id)),
            )
        {
            return;
        }
        // Loading exposes no actions. Only this completion can replace it.
        self.navigation = Some(Page::Loading);
        self.message = "";
        window.focus(&self.navigation_focus, cx);
        let identifier = Login::identifier(window, cx);
        let work = cx.background_executor().spawn(async move {
            match board {
                Some(id) => client.read_selected_board(id).map(Page::Selected),
                None => client.list_boards().map(Page::List),
            }
        });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            _ = view.update(cx, |view, cx| {
                if !matches!(view.navigation, Some(Page::Loading)) {
                    return;
                }
                match result {
                    Ok(page) => view.navigation = Some(page),
                    Err(EditError::SessionRejected) => view.forget_session(conn, identifier, cx),
                    Err(EditError::PermissionDenied | EditError::Missing) if board.is_some() => {
                        view.navigation = Some(Page::Denied)
                    }
                    Err(_) => view.navigation = Some(Page::Fault),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn navigation_page(
        &self,
        page: &Page,
        mut column: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        let scroller = div()
            .id("browse-scroll")
            .size_full()
            .overflow_y_scroll()
            .track_focus(&self.navigation_focus)
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, w, cx| {
                if event.keystroke.key == "escape"
                    && matches!(
                        view.navigation,
                        Some(Page::Selected(_) | Page::Denied | Page::Fault)
                    )
                {
                    view.navigate(None, w, cx);
                    cx.stop_propagation();
                }
            }));
        let action = match page {
            Page::Loading => {
                return div()
                    .size_full()
                    .child(scroller.child(column.child(Label::new("Loading…"))));
            }
            Page::List(list) => {
                column = column.child(
                    Label::new(if list.is_empty() {
                        "No boards"
                    } else {
                        "Boards"
                    })
                    .text_xl(),
                );
                for summary in list {
                    let id = summary.id;
                    column = column.child(
                        Button::new(SharedString::from(id.to_string()))
                            .large()
                            .flex_shrink_0()
                            .label(summary.name.clone())
                            .disabled(self.logging_out)
                            .on_click(
                                cx.listener(move |view, _, w, cx| view.navigate(Some(id), w, cx)),
                            ),
                    );
                }
                "Refresh"
            }
            Page::Selected(rows) => {
                column = column.child(Label::new("Board items").text_xl());
                if rows.is_empty() {
                    column = column.child(Label::new("Empty board"));
                }
                for row in rows {
                    column = column.child(Label::new(row.title.clone()));
                }
                "Back to boards"
            }
            Page::Denied => {
                column = column.child(Label::new("Destination unavailable"));
                "Back to boards"
            }
            Page::Fault => {
                column = column.child(Label::new("Could not load navigation"));
                "Retry board list"
            }
        };
        column = column.child(
            Button::new("boards")
                .large()
                .label(action)
                .disabled(self.logging_out)
                .on_click(cx.listener(|view, _, w, cx| view.navigate(None, w, cx))),
        );
        if !self.message.is_empty() {
            column = column.child(Label::new(self.message));
        }
        column = column.child(self.logout_button(cx));
        div().size_full().child(scroller.child(column.h_auto()))
    }
}
