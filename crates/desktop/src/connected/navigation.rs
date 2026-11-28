use super::main_table::MainTable;
use super::*;
use boardcraft_mobile_edit_client::{
    Automation, BoardSummary, DirectoryUser, InboxNotification, MainTableRead, SavedView,
};
use gpui_kit::component::input::InputEvent;

enum ReadPage {
    List(Vec<BoardSummary>, i64),
    Table(MainTableRead, Vec<SavedView>, Vec<Automation>, i64),
}

pub(super) struct PeoplePage {
    pub(super) users: Vec<DirectoryUser>,
    pub(super) query: Entity<InputState>,
    pub(super) pending: bool,
    pub(super) message: &'static str,
}

pub(super) struct InboxPage {
    unread: i64,
    notifications: Vec<InboxNotification>,
    pending: bool,
    message: &'static str,
}

pub(super) enum Page {
    List(Vec<BoardSummary>),
    People(PeoplePage),
    Items(Uuid, Box<MainTable>),
    Inbox(InboxPage),
    Recover(Uuid, FocusHandle),
    Unavailable,
    Fault(Option<Uuid>),
}

pub(super) struct Navigation {
    pub(super) client: EditClient,
    pub(super) page: Page,
    pub(super) unread: i64,
    pub(super) share: Option<Entity<InputState>>,
    pub(super) _people_events: Vec<Subscription>,
}

impl Connected {
    pub(super) fn navigate(
        &mut self,
        client: EditClient,
        board: Option<Uuid>,
        item: Option<Uuid>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.screen = Screen::Loading;
        self.message = "";
        self.navigation_request += 1;
        let request = self.navigation_request;
        let work = cx.background_executor().spawn(async move {
            let result = match board {
                Some(id) => client.read_main_table(id).and_then(|read| {
                    let views = client.list_views(id)?;
                    let automations = client.list_automations(id)?;
                    let unread = client.list_notifications(false)?.unread;
                    Ok(ReadPage::Table(read, views, automations, unread))
                }),
                None => client.resolve().and_then(|()| {
                    let list = client.list_boards()?;
                    let unread = client.list_notifications(false)?.unread;
                    Ok(ReadPage::List(list, unread))
                }),
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
                    let (page, unread) = match result {
                        Ok(ReadPage::List(list, unread)) => (Page::List(list), unread),
                        Ok(ReadPage::Table(read, saved, rules, unread)) => {
                            let board = board.expect("selected board");
                            let mut table = MainTable::new(read, board, None, w, cx);
                            table.views = saved;
                            table.automations = rules;
                            table.focus_first(w, cx);
                            if let Some(item) = item {
                                table.focus_item(item, w, cx);
                            }
                            (Page::Items(board, Box::new(table)), unread)
                        }
                        Err(EditError::PermissionDenied | EditError::Missing)
                            if board.is_some() =>
                        {
                            (Page::Unavailable, 0)
                        }
                        Err(_) => (Page::Fault(board), 0),
                    };
                    view.link_input.get_or_insert_with(|| {
                        cx.new(|cx| InputState::new(w, cx).placeholder("Paste board address"))
                    });
                    view.screen = Screen::Navigation(Navigation {
                        client,
                        page,
                        unread,
                        share: None,
                        _people_events: Vec::new(),
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
        self.navigate(nav.client.clone(), board, None, w, cx);
    }

    fn open_people(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        let client = nav.client.clone();
        let query = cx.new(|cx| InputState::new(w, cx).placeholder("Search people"));
        let events = cx.subscribe_in(&query, w, |view, _, event, w, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                view.search_people(w, cx);
            }
        });
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        nav._people_events = vec![events];
        nav.page = Page::People(PeoplePage {
            users: Vec::new(),
            query: query.clone(),
            pending: true,
            message: "",
        });
        query.update(cx, |input, cx| input.focus(w, cx));
        self.load_people(client, "", w, cx);
    }

    fn search_people(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let (client, query) = {
            let Screen::Navigation(nav) = &mut self.screen else {
                return;
            };
            let Page::People(page) = &mut nav.page else {
                return;
            };
            if page.pending {
                return;
            }
            page.pending = true;
            page.message = "";
            (nav.client.clone(), page.query.read(cx).value().to_string())
        };
        self.load_people(client, &query, w, cx);
    }

    fn load_people(
        &mut self,
        client: EditClient,
        query: &str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let query = query.to_owned();
        let request = self.navigation_request;
        self.message = "";
        let work = cx
            .background_executor()
            .spawn(async move { client.list_users(&query) });
        cx.spawn_in(w, async move |view, cx| {
            let result = work.await;
            _ = view.update_in(cx, |view, w, cx| {
                if request != view.navigation_request {
                    return;
                }
                let Screen::Navigation(nav) = &mut view.screen else {
                    return;
                };
                let Page::People(page) = &mut nav.page else {
                    return;
                };
                match result {
                    Ok(users) => {
                        page.users = users;
                        page.pending = false;
                        page.message = if page.users.is_empty() {
                            "No people found"
                        } else {
                            ""
                        };
                    }
                    Err(EditError::SessionRejected) => {
                        view.link_input = None;
                        view.message = "Session expired. Sign in again.";
                        view.screen = Self::identifier(w, cx);
                        return;
                    }
                    Err(_) => {
                        page.pending = false;
                        page.message = "Could not load people.";
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn open_inbox(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let client = nav.client.clone();
        let unread = nav.unread;
        nav.page = Page::Inbox(InboxPage {
            unread,
            notifications: Vec::new(),
            pending: true,
            message: "",
        });
        self.navigation_request += 1;
        self.load_inbox(client, w, cx);
    }

    fn load_inbox(&mut self, client: EditClient, w: &mut Window, cx: &mut Context<Self>) {
        let request = self.navigation_request;
        self.message = "";
        let work = cx.background_executor().spawn(async move {
            let inbox = client.list_notifications(false)?;
            let unseen: Vec<Uuid> = inbox
                .notifications
                .iter()
                .filter(|notification| {
                    notification.read_at.is_none() && notification.seen_at.is_none()
                })
                .map(|notification| notification.id)
                .collect();
            if unseen.is_empty() {
                return Ok(inbox);
            }
            for ids in unseen.chunks(boardcraft_board::notifications::SEEN_LIMIT) {
                client.mark_notifications_seen(ids)?;
            }
            client.list_notifications(false)
        });
        cx.spawn_in(w, async move |view, cx| {
            let result = work.await;
            _ = view.update_in(cx, |view, w, cx| {
                if request != view.navigation_request {
                    return;
                }
                let Screen::Navigation(nav) = &mut view.screen else {
                    return;
                };
                let Page::Inbox(page) = &mut nav.page else {
                    return;
                };
                match result {
                    Ok(inbox) => {
                        nav.unread = inbox.unread;
                        page.unread = inbox.unread;
                        page.notifications = inbox.notifications;
                        page.pending = false;
                        page.message = "";
                    }
                    Err(EditError::SessionRejected) => {
                        view.link_input = None;
                        view.message = "Session expired. Sign in again.";
                        view.screen = Self::identifier(w, cx);
                        return;
                    }
                    Err(_) => {
                        page.pending = false;
                        page.message = "Could not load notifications.";
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn mark_notification(&mut self, id: Uuid, read: bool, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Inbox(page) = &mut nav.page else {
            return;
        };
        if page.pending {
            return;
        }
        page.pending = true;
        page.message = "";
        let client = nav.client.clone();
        let request = self.navigation_request;
        let work = cx
            .background_executor()
            .spawn(async move { client.set_notification_read(id, read).map(|_| ()) });
        cx.spawn_in(w, async move |view, cx| {
            let result = work.await;
            _ = view.update_in(cx, |view, w, cx| {
                if request != view.navigation_request {
                    return;
                }
                if matches!(result, Err(EditError::SessionRejected)) {
                    view.link_input = None;
                    view.message = "Session expired. Sign in again.";
                    view.screen = Self::identifier(w, cx);
                    cx.notify();
                    return;
                }
                if result.is_ok() {
                    let Screen::Navigation(nav) = &view.screen else {
                        return;
                    };
                    let client = nav.client.clone();
                    view.load_inbox(client, w, cx);
                    return;
                }
                let Screen::Navigation(nav) = &mut view.screen else {
                    return;
                };
                let Page::Inbox(page) = &mut nav.page else {
                    return;
                };
                page.pending = false;
                page.message = "Could not update notification.";
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn delete_notification(&mut self, id: Uuid, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Inbox(page) = &mut nav.page else {
            return;
        };
        if page.pending {
            return;
        }
        page.pending = true;
        page.message = "";
        let client = nav.client.clone();
        let request = self.navigation_request;
        let work = cx
            .background_executor()
            .spawn(async move { client.delete_notification(id).map(|_| ()) });
        cx.spawn_in(w, async move |view, cx| {
            let result = work.await;
            _ = view.update_in(cx, |view, w, cx| {
                if request != view.navigation_request {
                    return;
                }
                if matches!(result, Err(EditError::SessionRejected)) {
                    view.link_input = None;
                    view.message = "Session expired. Sign in again.";
                    view.screen = Self::identifier(w, cx);
                    cx.notify();
                    return;
                }
                if result.is_ok() {
                    let Screen::Navigation(nav) = &view.screen else {
                        return;
                    };
                    let client = nav.client.clone();
                    view.load_inbox(client, w, cx);
                    return;
                }
                let Screen::Navigation(nav) = &mut view.screen else {
                    return;
                };
                let Page::Inbox(page) = &mut nav.page else {
                    return;
                };
                page.pending = false;
                page.message = "Could not delete notification.";
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn mark_all_notifications_read(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Inbox(page) = &mut nav.page else {
            return;
        };
        if page.pending {
            return;
        }
        page.pending = true;
        page.message = "";
        let client = nav.client.clone();
        let request = self.navigation_request;
        let work = cx
            .background_executor()
            .spawn(async move { client.mark_all_notifications_read().map(|_| ()) });
        cx.spawn_in(w, async move |view, cx| {
            let result = work.await;
            _ = view.update_in(cx, |view, w, cx| {
                if request != view.navigation_request {
                    return;
                }
                if matches!(result, Err(EditError::SessionRejected)) {
                    view.link_input = None;
                    view.message = "Session expired. Sign in again.";
                    view.screen = Self::identifier(w, cx);
                    cx.notify();
                    return;
                }
                if result.is_ok() {
                    let Screen::Navigation(nav) = &view.screen else {
                        return;
                    };
                    let client = nav.client.clone();
                    view.load_inbox(client, w, cx);
                    return;
                }
                let Screen::Navigation(nav) = &mut view.screen else {
                    return;
                };
                let Page::Inbox(page) = &mut nav.page else {
                    return;
                };
                page.pending = false;
                page.message = "Could not mark all as read.";
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn open_notification(
        &mut self,
        id: Uuid,
        board: Uuid,
        item: Uuid,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let client = nav.client.clone();
        let Page::Inbox(page) = &mut nav.page else {
            return;
        };
        if page.pending {
            return;
        }
        page.pending = true;
        page.message = "";
        self.navigation_request += 1;
        let request = self.navigation_request;
        let read = client.clone();
        let work = cx
            .background_executor()
            .spawn(async move { read.set_notification_read(id, true).map(|_| ()) });
        cx.spawn_in(w, async move |view, cx| {
            let result = work.await;
            _ = view.update_in(cx, |view, w, cx| {
                if request != view.navigation_request {
                    return;
                }
                if matches!(result, Err(EditError::SessionRejected)) {
                    view.link_input = None;
                    view.message = "Session expired. Sign in again.";
                    view.screen = Self::identifier(w, cx);
                    cx.notify();
                    return;
                }
                view.navigate(client, Some(board), Some(item), w, cx);
            });
        })
        .detach();
        cx.notify();
    }

    fn reload_main_table(&mut self, board: Uuid, w: &mut Window, cx: &mut Context<Self>) {
        if self.refresh_item_entry(board, true, w, cx) {
            return;
        }
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        self.navigate(nav.client.clone(), Some(board), None, w, cx);
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
                    view.navigate(client, Some(id), None, w, cx);
                    view
                })
            });
            if opened.is_err() {
                self.message = "Could not open window. Try again.";
                cx.notify();
            }
        } else {
            self.navigate(client, Some(id), None, w, cx);
        }
    }
}

fn role_label(role: &str) -> String {
    let mut chars = role.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn notifications_label(unread: i64) -> String {
    if unread > 0 {
        format!("Notifications ({unread})")
    } else {
        "Notifications".to_owned()
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
                column = column.child(
                    Button::new("notifications")
                        .label(notifications_label(self.unread))
                        .on_click(cx.listener(|view, _, w, cx| view.open_inbox(w, cx))),
                );
                column = column.child(
                    Button::new("people")
                        .label("People")
                        .on_click(cx.listener(|view, _, w, cx| view.open_people(w, cx))),
                );
                "Refresh"
            }
            Page::People(people) => {
                column = column.child(Label::new("People")).child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Input::new(&people.query).id("people-search"))
                        .child(
                            Button::new("people-search-go")
                                .label("Search")
                                .disabled(people.pending)
                                .on_click(cx.listener(|view, _, w, cx| view.search_people(w, cx))),
                        ),
                );
                if people.pending {
                    column = column.child(Label::new("Loading people…"));
                }
                if !people.message.is_empty() {
                    column = column.child(Label::new(people.message));
                }
                for user in &people.users {
                    let mut line = format!(
                        "{} · {} · {}",
                        user.username,
                        user.email,
                        role_label(&user.role)
                    );
                    if !user.active {
                        line.push_str(" · Inactive");
                    }
                    if !user.teams.is_empty() {
                        line.push_str(&format!(" · {}", user.teams.join(", ")));
                    }
                    column = column.child(
                        Label::new(line).id(SharedString::from(format!("person-{}", user.id))),
                    );
                }
                "Back to boards"
            }
            Page::Items(id, table) => {
                let id = *id;
                column = column.child(table.render(cx));
                column = column.child(
                    Button::new("reload-table")
                        .label("Reload board")
                        .disabled(table.is_pending())
                        .on_click(
                            cx.listener(move |view, _, w, cx| view.reload_main_table(id, w, cx)),
                        ),
                );
                column = column.child(
                    Button::new("notifications")
                        .label(notifications_label(self.unread))
                        .on_click(cx.listener(|view, _, w, cx| view.open_inbox(w, cx))),
                );
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
            Page::Inbox(page) => {
                column = column
                    .child(Label::new("Notifications"))
                    .child(Label::new(if page.unread == 0 {
                        "You're all caught up".to_owned()
                    } else {
                        format!("{} unread", page.unread)
                    }))
                    .child(
                        Button::new("notifications-read-all")
                            .label("Mark all as read")
                            .disabled(page.pending || page.unread == 0)
                            .on_click(cx.listener(|view, _, w, cx| {
                                view.mark_all_notifications_read(w, cx)
                            })),
                    );
                if page.pending {
                    column = column.child(Label::new("Loading notifications…"));
                }
                if !page.message.is_empty() {
                    column = column.child(Label::new(page.message));
                }
                for notification in &page.notifications {
                    let id = notification.id;
                    let board = notification.board_id;
                    let item = notification.item_id;
                    let mark_read = notification.read_at.is_none();
                    let mut heading = div()
                        .id(SharedString::from(format!("notification-heading-{id}")))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Label::new(notification.title.clone())
                                .id(SharedString::from(format!("notification-title-{id}"))),
                        );
                    if notification.read_at.is_none() {
                        heading = heading.child(
                            Label::new("Unread")
                                .id(SharedString::from(format!("notification-unread-{id}"))),
                        );
                    }
                    column =
                        column.child(
                            div()
                                .id(SharedString::from(format!("notification-{id}")))
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(heading)
                                .child(
                                    Label::new(notification.message.clone()).id(
                                        SharedString::from(format!("notification-message-{id}")),
                                    ),
                                )
                                .child(
                                    Label::new(format!(
                                        "{} — {}",
                                        notification.board_name, notification.item_name
                                    ))
                                    .id(SharedString::from(format!("notification-context-{id}"))),
                                )
                                .child(
                                    Label::new(notification.created_at.clone()).id(
                                        SharedString::from(format!("notification-created-{id}")),
                                    ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "notification-read-{id}"
                                            )))
                                            .label(if mark_read {
                                                "Mark read"
                                            } else {
                                                "Mark unread"
                                            })
                                            .disabled(page.pending)
                                            .on_click(cx.listener(move |view, _, w, cx| {
                                                view.mark_notification(id, mark_read, w, cx)
                                            })),
                                        )
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "notification-delete-{id}"
                                            )))
                                            .label("Delete")
                                            .disabled(page.pending)
                                            .on_click(cx.listener(move |view, _, w, cx| {
                                                view.delete_notification(id, w, cx)
                                            })),
                                        )
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "notification-open-{id}"
                                            )))
                                            .label("Open")
                                            .disabled(page.pending)
                                            .on_click(cx.listener(move |view, _, w, cx| {
                                                view.open_notification(id, board, item, w, cx)
                                            })),
                                        ),
                                ),
                        );
                }
                "Back to boards"
            }
            Page::Recover(id, focus) => {
                let id = *id;
                column = column.child(
                    Button::new("reload-table")
                        .label("Reload board")
                        .track_focus(focus)
                        .on_click(
                            cx.listener(move |view, _, w, cx| view.reload_main_table(id, w, cx)),
                        ),
                );
                "Back to boards"
            }
            Page::Unavailable => {
                column = column.child(Label::new("Destination unavailable"));
                "Back to boards"
            }
            Page::Fault(Some(id)) => {
                let id = *id;
                column = column
                    .child(Label::new("Could not load main table."))
                    .child(Button::new("reload-table").label("Reload board").on_click(
                        cx.listener(move |view, _, w, cx| view.reload_main_table(id, w, cx)),
                    ));
                "Back to boards"
            }
            Page::Fault(None) => {
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
