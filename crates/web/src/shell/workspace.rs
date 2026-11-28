//! The navigation view and its independent, scrollable board detail pane.

use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, Role, StatefulInteractiveElement,
    Styled,
    base::Disableable,
    component::{ActiveTheme, button::Button},
    div,
};

use super::{Page, Shell, status};
use boardcraft_ui::{BoardHeader, BoardTabs};

impl Shell {
    pub(super) fn workspace(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match self.destination {
            None => "Boards",
            Some(id) => self
                .boards
                .as_deref()
                .unwrap_or_default()
                .iter()
                .find(|board| board.id == id)
                .map_or("Board", |board| board.name.as_str()),
        };
        let mut content = div().flex().flex_col().items_start().gap_3();
        content = match &self.page {
            Page::Board(table) => content
                .size_full()
                .min_h_0()
                .items_stretch()
                .child(table.clone()),
            Page::List => {
                if let Some(boards) = &self.boards {
                    content = content
                        .child(status(
                            "page-status",
                            if boards.is_empty() {
                                "No boards"
                            } else {
                                "Choose a board from the sidebar."
                            },
                        ))
                        .child(
                            Button::new("refresh")
                                .label("Refresh")
                                .disabled(self.logging_out)
                                .on_click(cx.listener(|s, _, w, cx| s.check(w, cx))),
                        );
                }
                content
            }
            Page::Loading => content.child(status(
                "page-status",
                if self.destination.is_some() {
                    "Loading board…"
                } else {
                    "Loading boards…"
                },
            )),
            Page::BoardUnavailable | Page::Denied => content
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
                        .disabled(self.logging_out)
                        .on_click(cx.listener(|s, _, w, cx| s.check(w, cx))),
                ),
        };
        div()
            .id("web-shell")
            .role(Role::Application)
            .aria_label("boardcraft")
            .track_focus(&self.focus)
            .flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.navigation.clone())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_shrink_0()
                            .px_6()
                            .py_2()
                            .child(BoardHeader::new(title.to_owned())),
                    )
                    .children(
                        matches!(self.page, Page::Board(_))
                            .then(|| div().px_6().pb_3().child(BoardTabs::new())),
                    )
                    .child(if matches!(self.page, Page::Board(_)) {
                        div().flex_1().min_h_0().child(content).into_any_element()
                    } else {
                        div()
                            .id("content-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .child(div().px_6().pb_6().child(content))
                            .into_any_element()
                    })
                    .children((!self.message.is_empty()).then(|| status("notice", self.message))),
            )
    }
}
