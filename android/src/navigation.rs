use super::*;
use boardcraft_mobile_edit_client::{Automation, BoardSummary, SavedView, view_filter_matches};

pub(super) enum Page {
    List(Vec<BoardSummary>),
    Loading,
    Selected {
        board: Uuid,
        rows: Vec<SavedItem>,
        views: Vec<SavedView>,
        automations: Vec<Automation>,
        filter: String,
    },
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
                Some(id) => client.read_selected_board(id).and_then(|rows| {
                    let views = client.list_views(id)?;
                    let automations = client.list_automations(id)?;
                    Ok(Page::Selected {
                        board: id,
                        rows,
                        views,
                        automations,
                        filter: String::new(),
                    })
                }),
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

    /// Applying a saved view only replaces the board's active filter; the rows
    /// stay loaded, so switching between views needs no further board read.
    fn apply_saved_view(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let Some(Page::Selected { views, filter, .. }) = &mut self.navigation else {
            return;
        };
        let Some(view) = views.iter().find(|view| view.id == id) else {
            return;
        };
        *filter = view.filter.clone();
        cx.notify();
    }

    /// Rules are listed as read-only rows; only the name input and create
    /// action mutate, matching the connected desktop table.
    fn render_automations(
        &self,
        automations: &[Automation],
        editable: bool,
        board: Uuid,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut panel = div()
            .id("automations")
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new("Automations"));
        if editable {
            panel = panel.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Input::new(&self.automation_name)
                            .id("automation-name")
                            .large()
                            .disabled(self.automation_pending || self.logging_out),
                    )
                    .child(
                        Button::new("create-automation")
                            .large()
                            .flex_shrink_0()
                            .label(if self.automation_pending {
                                "Creating…"
                            } else {
                                "Create automation"
                            })
                            .disabled(self.automation_pending || self.logging_out)
                            .on_click(cx.listener(move |view, _, w, cx| {
                                view.create_automation(board, w, cx);
                            })),
                    ),
            );
        }
        if automations.is_empty() {
            panel = panel.child(Label::new("No automations"));
        }
        for rule in automations {
            let id = rule.id;
            let last = rule
                .last_item_title
                .as_ref()
                .map(|title| format!("Last run: {title}"))
                .unwrap_or_else(|| "Never run".to_owned());
            panel = panel.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        Label::new(rule.name.clone())
                            .id(SharedString::from(format!("automation-{id}"))),
                    )
                    .child(
                        Label::new("When item renamed → Record a run")
                            .id(SharedString::from(format!("automation-rule-{id}"))),
                    )
                    .child(
                        Label::new(last).id(SharedString::from(format!("automation-last-{id}"))),
                    ),
            );
        }
        panel
    }

    fn create_automation(&mut self, board: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(client)) = (self.connection, self.client.clone()) else {
            return;
        };
        if self.logging_out
            || self.automation_pending
            || !matches!(
                &self.navigation,
                Some(Page::Selected { board: current, .. }) if *current == board
            )
        {
            return;
        }
        let name = self.automation_name.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.message = "Enter an automation name.";
            cx.notify();
            return;
        }
        self.message = "";
        self.automation_pending = true;
        let identifier = Login::identifier(window, cx);
        let work = cx
            .background_executor()
            .spawn(async move { client.create_automation(board, Uuid::new_v4(), &name) });
        cx.spawn_in(window, async move |view, cx| {
            let result = work.await;
            _ = view.update_in(cx, |view, window, cx| {
                view.automation_pending = false;
                if matches!(&result, Err(EditError::SessionRejected)) {
                    view.forget_session(conn, identifier, cx);
                    cx.notify();
                    return;
                }
                let message = match result {
                    Ok(saved) => {
                        let Some(Page::Selected {
                            board: current,
                            automations,
                            ..
                        }) = &mut view.navigation
                        else {
                            return;
                        };
                        if *current != board {
                            return;
                        }
                        automations.retain(|rule| rule.id != saved.id);
                        automations.push(saved);
                        automations.sort_by_key(|rule| rule.name.to_lowercase());
                        view.automation_name
                            .update(cx, |input, cx| input.set_value("", window, cx));
                        "Automation created"
                    }
                    Err(EditError::Conflict) => "An automation with that name already exists.",
                    Err(EditError::InvalidDraft) => "Enter an automation name.",
                    Err(EditError::PermissionDenied) => {
                        "No permission to create automations on this board."
                    }
                    Err(EditError::Missing) => "Board not found. Back to boards.",
                    Err(EditError::AccessUnavailable) => "Access check unavailable. Try again.",
                    Err(_) => "Could not create the automation.",
                };
                view.message = message;
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
                        Some(Page::Selected { .. } | Page::Denied | Page::Fault)
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
            Page::Selected {
                board,
                rows,
                views,
                automations,
                filter,
            } => {
                column = column.child(Label::new("Board items").text_xl());
                if !views.is_empty() {
                    let mut saved = div().flex().flex_wrap().gap_2();
                    for view in views {
                        let id = view.id;
                        saved = saved.child(
                            Button::new(SharedString::from(format!("view-{id}")))
                                .large()
                                .flex_shrink_0()
                                .label(view.name.clone())
                                .disabled(self.logging_out)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.apply_saved_view(id, cx);
                                })),
                        );
                    }
                    column = column.child(Label::new("Views")).child(saved);
                }
                if !filter.is_empty() {
                    column = column.child(Label::new(format!("Filter: {filter}")));
                }
                column = column.child(self.render_automations(
                    automations,
                    self.connection.is_some(),
                    *board,
                    cx,
                ));
                let visible = rows
                    .iter()
                    .filter(|row| view_filter_matches(filter, &row.title))
                    .count();
                if visible == 0 {
                    column = column.child(Label::new(if rows.is_empty() {
                        "Empty board"
                    } else {
                        "No items"
                    }));
                }
                for row in rows
                    .iter()
                    .filter(|row| view_filter_matches(filter, &row.title))
                {
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
