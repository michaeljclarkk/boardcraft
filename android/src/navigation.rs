use super::*;
use gpui_kit::{
    base::Selectable,
    component::menu::{DropdownMenu, PopupMenuItem},
};
mod activity;
mod status;
use activity::ItemActivity;
use boardcraft_mobile_edit_client::{
    Automation, BoardSummary, InboxNotification, MainTableGroup, NotificationInbox, SavedView,
    view_filter_matches,
};
use status::StatusColumn;

/// The host accepts at most this many ids in one seen request.
const SEEN_BATCH: usize = 100;

pub(super) enum Page {
    List(Vec<BoardSummary>),
    Loading,
    Selected {
        board: Uuid,
        rows: Vec<SavedItem>,
        groups: Vec<MainTableGroup>,
        status: StatusColumn,
        views: Vec<SavedView>,
        automations: Vec<Automation>,
        filter: String,
    },
    Inbox(Inbox),
    Activity(ItemActivity),
    Denied,
    Fault,
}

pub(super) struct Inbox {
    notifications: Vec<InboxNotification>,
    unread: i64,
    loading: bool,
    /// False until one load succeeded; a failed load keeps the empty page
    /// from claiming the inbox is caught up.
    loaded: bool,
    pending: bool,
}

impl Inbox {
    fn apply(&mut self, mut loaded: NotificationInbox) {
        // Newest first. The host already orders by created_at then id; a stable
        // sort keeps that order for rows created in the same minute.
        loaded
            .notifications
            .sort_by(|left, right| right.created_at.cmp(&left.created_at));
        self.notifications = loaded.notifications;
        self.unread = loaded.unread;
        self.loading = false;
        self.loaded = true;
        self.pending = false;
    }
}

impl Page {
    /// A page that owns an in-flight action must settle before logout.
    pub(super) fn pending(&self) -> bool {
        match self {
            Page::Loading => true,
            Page::Activity(activity) => activity.loading(),
            Page::Inbox(inbox) => inbox.pending,
            Page::Selected { status, .. } => status.pending.is_some(),
            _ => false,
        }
    }
}

#[derive(Clone, Copy)]
enum InboxAction {
    Read(Uuid, bool),
    Delete(Uuid),
    ReadAll,
}

/// The opened inbox records its unread rows as seen once. That receipt does not
/// change what the inbox shows, so a failure to record it still returns rows.
fn load_inbox(client: &EditClient) -> Result<NotificationInbox, EditError> {
    let mut inbox = client.list_notifications(false)?;
    let ids: Vec<Uuid> = inbox
        .notifications
        .iter()
        .filter(|row| row.read_at.is_none() && row.seen_at.is_none())
        .map(|row| row.id)
        .collect();
    if !ids.is_empty() {
        let marked = ids
            .chunks(SEEN_BATCH)
            .all(|batch| client.mark_notifications_seen(batch).unwrap_or(false));
        if marked {
            let seen = seen_marker();
            for row in &mut inbox.notifications {
                if ids.contains(&row.id) {
                    row.seen_at = Some(seen.clone());
                }
            }
        }
    }
    Ok(inbox)
}

/// Local receipt marker; the host returned no timestamp with the seen reply.
fn seen_marker() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs().to_string())
        .unwrap_or_default()
}

fn inbox_message(error: EditError) -> &'static str {
    match error {
        EditError::PermissionDenied => "No permission to view notifications.",
        EditError::AccessUnavailable => "Access check unavailable. Try again.",
        _ => "Could not load notifications.",
    }
}

impl BoardView {
    pub(super) fn navigate(
        &mut self,
        board: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(conn), Some(client), Some(page)) =
            (self.connection, self.client.clone(), &self.navigation)
        else {
            return;
        };
        // A board is reachable only from the screen that named it: a listed
        // board, or the inbox row that points at it.
        let reachable = match board {
            None => true,
            Some(id) => match page {
                Page::List(boards) => boards.iter().any(|board| board.id == id),
                Page::Inbox(inbox) => inbox.notifications.iter().any(|row| row.board_id == id),
                Page::Activity(activity) => activity.board == id,
                Page::Selected { board, .. } => *board == id,
                _ => false,
            },
        };
        if self.logging_out || (page.pending() && !matches!(page, Page::Activity(_))) || !reachable
        {
            return;
        }
        if let Some(id) = board {
            let title = match page {
                Page::List(list) => list
                    .iter()
                    .find(|board| board.id == id)
                    .map(|board| board.name.clone()),
                Page::Inbox(inbox) => inbox
                    .notifications
                    .iter()
                    .find(|row| row.board_id == id)
                    .map(|row| row.board_name.clone()),
                _ => None,
            };
            if let Some(title) = title {
                self.navigation_title = title;
            }
        }
        let retained_filter = match (&self.navigation, board) {
            (
                Some(Page::Selected {
                    board: current,
                    filter,
                    ..
                }),
                Some(next),
            ) if *current == next => Some(filter.clone()),
            _ => None,
        };
        if retained_filter.is_none() {
            self.search_task = None;
            self.result_search.clear();
            self.search_pending = false;
            self.search_invalid = false;
            self.folded.clear();
            self.selected.clear();
            self.board_search
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.views_open = false;
            self.automations_open = false;
        }
        // Loading exposes no actions. Only this completion can replace it.
        self.navigation = Some(Page::Loading);
        self.inbox_unread = None;
        self.message = "";
        window.focus(&self.navigation_focus, cx);
        let identifier = Login::identifier(window, cx);
        let work = cx.background_executor().spawn(async move {
            match board {
                Some(id) => StatusColumn::load(&client, id).and_then(|(rows, groups, status)| {
                    let views = client.list_views(id)?;
                    let automations = client.list_automations(id)?;
                    Ok(Page::Selected {
                        board: id,
                        rows,
                        groups,
                        status,
                        views,
                        automations,
                        filter: retained_filter.unwrap_or_default(),
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
                    Ok(mut page) => {
                        if let Page::Selected {
                            rows,
                            groups,
                            filter,
                            ..
                        } = &mut page
                        {
                            // A pending find can settle while Reload is reading the board.
                            // Use that accepted query rather than the older request snapshot.
                            *filter = view.result_search.clone();
                            view.selected
                                .retain(|id| rows.iter().any(|row| row.id == *id));
                            view.folded
                                .retain(|id, _| groups.iter().any(|group| group.id() == id));
                        }
                        view.navigation = Some(page);
                    }
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

    /// The board list names the inbox and its count. Loading it is best effort:
    /// a failed count leaves the button without a number, and the inbox itself
    /// reports its own result.
    pub(super) fn ensure_inbox_count(&mut self, cx: &mut Context<Self>) {
        if self.inbox_count_pending
            || self.inbox_unread.is_some()
            || self.logging_out
            || !matches!(self.navigation, Some(Page::List(_)))
        {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        self.inbox_count_pending = true;
        let work = cx
            .background_executor()
            .spawn(async move { client.list_notifications(false).map(|inbox| inbox.unread) });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            _ = view.update(cx, |view, cx| {
                view.inbox_count_pending = false;
                if !matches!(view.navigation, Some(Page::List(_))) {
                    return;
                }
                view.inbox_unread = Some(result.unwrap_or(0));
                cx.notify();
            });
        })
        .detach();
    }

    fn open_inbox(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(client)) = (self.connection, self.client.clone()) else {
            return;
        };
        if self.logging_out
            || self.login.is_some()
            || matches!(self.navigation, Some(Page::Loading | Page::Inbox(_)))
        {
            return;
        }
        self.navigation = Some(Page::Inbox(Inbox {
            notifications: Vec::new(),
            unread: 0,
            loading: true,
            loaded: false,
            pending: true,
        }));
        self.message = "";
        window.focus(&self.navigation_focus, cx);
        let identifier = Login::identifier(window, cx);
        let work = cx
            .background_executor()
            .spawn(async move { load_inbox(&client) });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            _ = view.update(cx, |view, cx| {
                if !matches!(view.navigation, Some(Page::Inbox(_))) {
                    return;
                }
                match result {
                    Ok(loaded) => {
                        if let Some(Page::Inbox(inbox)) = &mut view.navigation {
                            inbox.apply(loaded);
                        }
                    }
                    Err(EditError::SessionRejected) => {
                        view.forget_session(conn, identifier, cx);
                        return;
                    }
                    Err(error) => {
                        if let Some(Page::Inbox(inbox)) = &mut view.navigation {
                            inbox.loading = false;
                            inbox.pending = false;
                        }
                        view.message = inbox_message(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Each action runs on a worker and then reloads the inbox once. While it is
    /// in flight every inbox action is disabled, so two rows cannot race.
    fn inbox_action(&mut self, action: InboxAction, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(client)) = (self.connection, self.client.clone()) else {
            return;
        };
        let pending = matches!(&self.navigation, Some(Page::Inbox(inbox)) if inbox.pending);
        if self.logging_out || pending {
            return;
        }
        let Some(Page::Inbox(inbox)) = &mut self.navigation else {
            return;
        };
        inbox.pending = true;
        self.message = "";
        let identifier = Login::identifier(window, cx);
        let work = cx.background_executor().spawn(async move {
            match action {
                InboxAction::Read(id, read) => client.set_notification_read(id, read).map(drop),
                InboxAction::Delete(id) => client.delete_notification(id).map(drop),
                InboxAction::ReadAll => client.mark_all_notifications_read().map(drop),
            }?;
            load_inbox(&client)
        });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            _ = view.update(cx, |view, cx| {
                if !matches!(view.navigation, Some(Page::Inbox(_))) {
                    return;
                }
                match result {
                    Ok(loaded) => {
                        if let Some(Page::Inbox(inbox)) = &mut view.navigation {
                            inbox.apply(loaded);
                        }
                    }
                    Err(EditError::SessionRejected) => {
                        view.forget_session(conn, identifier, cx);
                        return;
                    }
                    Err(error) => {
                        if let Some(Page::Inbox(inbox)) = &mut view.navigation {
                            inbox.pending = false;
                        }
                        view.message = match error {
                            EditError::PermissionDenied => "No permission to change notifications.",
                            EditError::Missing => "Notification not found.",
                            EditError::AccessUnavailable => "Access check unavailable. Try again.",
                            _ => "Could not update notifications.",
                        };
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Applying a saved view only replaces the board's active filter; the rows
    /// stay loaded, so switching between views needs no further board read.
    fn apply_saved_view(&mut self, id: Uuid, w: &mut Window, cx: &mut Context<Self>) {
        let Some(Page::Selected { views, filter, .. }) = &mut self.navigation else {
            return;
        };
        let Some(view) = views.iter().find(|view| view.id == id) else {
            return;
        };
        *filter = view.filter.clone();
        let value = filter.clone();
        self.search_task = None;
        self.result_search = value.clone();
        self.search_pending = false;
        self.search_invalid = false;
        self.board_search
            .update(cx, |input, cx| input.set_value(value, w, cx));
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
            .id(if matches!(page, Page::Activity(_)) {
                "activity-scroll"
            } else {
                "browse-scroll"
            })
            .size_full()
            .overflow_y_scroll()
            .track_focus(&self.navigation_focus)
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, w, cx| {
                if event.keystroke.key == "escape"
                    && let Some(Page::Activity(activity)) = &view.navigation
                {
                    view.navigate(Some(activity.board), w, cx);
                    cx.stop_propagation();
                    return;
                }
                if event.keystroke.key == "escape"
                    && matches!(
                        view.navigation,
                        Some(Page::Selected { .. } | Page::Inbox(_) | Page::Denied | Page::Fault)
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
                let unread = self.inbox_unread.unwrap_or(0);
                column = column.child(
                    Button::new("notifications")
                        .large()
                        .flex_shrink_0()
                        .label(if unread > 0 {
                            format!("Notifications ({unread})")
                        } else {
                            "Notifications".to_owned()
                        })
                        .disabled(self.logging_out)
                        .on_click(cx.listener(|view, _, w, cx| view.open_inbox(w, cx))),
                );
                "Refresh"
            }
            Page::Selected {
                board,
                rows,
                groups,
                status,
                views,
                automations,
                filter,
            } => {
                let busy = self.logging_out || self.automation_pending || status.pending.is_some();
                let owner = cx.weak_entity();
                let board_id = *board;
                column = column
                    .child(BoardHeader::new(self.navigation_title.clone()))
                    .child(BoardTabs::new())
                    .child(
                        BoardToolbar::new()
                            .action(
                                Button::new("views-toggle")
                                    .label("Views")
                                    .selected(self.views_open)
                                    .disabled(busy)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.views_open = !view.views_open;
                                        cx.notify();
                                    })),
                            )
                            .action(
                                Button::new("automations-toggle")
                                    .label("Automate")
                                    .selected(self.automations_open)
                                    .disabled(busy)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.automations_open = !view.automations_open;
                                        cx.notify();
                                    })),
                            )
                            .tools(
                                BoardToolbar::tools_trigger("board-tools-menu")
                                    .disabled(busy)
                                    .dropdown_menu(move |menu, _, _| {
                                        let clear = owner.clone();
                                        let reload = owner.clone();
                                        menu.item(PopupMenuItem::new("Clear filter").on_click(
                                            move |_, w, cx| {
                                                let _ = clear.update(cx, |view, cx| {
                                                    view.clear_search(false, w, cx);
                                                });
                                            },
                                        ))
                                        .item(
                                            PopupMenuItem::new("Reload board").on_click(
                                                move |_, w, cx| {
                                                    let _ = reload.update(cx, |view, cx| {
                                                        view.navigate(Some(board_id), w, cx)
                                                    });
                                                },
                                            ),
                                        )
                                    }),
                            ),
                    );
                if self.views_open && !views.is_empty() {
                    let mut saved = div().flex().flex_wrap().gap_2();
                    for view in views {
                        let id = view.id;
                        saved = saved.child(
                            Button::new(SharedString::from(format!("view-{id}")))
                                .large()
                                .flex_shrink_0()
                                .label(view.name.clone())
                                .disabled(self.logging_out)
                                .on_click(cx.listener(move |view, _, w, cx| {
                                    view.apply_saved_view(id, w, cx);
                                })),
                        );
                    }
                    column = column.child(Label::new("Views")).child(saved);
                }
                if self.automations_open {
                    column = column.child(self.render_automations(
                        automations,
                        status.editable && status.pending.is_none(),
                        *board,
                        cx,
                    ));
                }
                let visible = rows
                    .iter()
                    .filter(|row| view_filter_matches(filter, &row.title))
                    .count();
                if !filter.is_empty() {
                    column = column.child(Label::new(format!("{visible} matching items")));
                }
                if visible == 0 && !filter.is_empty() {
                    column = column
                        .child(Label::new("No results were found"))
                        .child(Label::new(
                            "Try another search or review this view’s filters.",
                        ));
                }
                for group in groups {
                    let matching: Vec<_> = group
                        .items()
                        .iter()
                        .filter_map(|item| rows.iter().find(|row| row.id == item.id))
                        .filter(|row| view_filter_matches(filter, &row.title))
                        .collect();
                    if !filter.is_empty() && matching.is_empty() {
                        continue;
                    }
                    let group_id = group.id().to_owned();
                    let folded = self
                        .folded
                        .get(&group_id)
                        .copied()
                        .unwrap_or(group.is_collapsed());
                    let count = matching.len();
                    let count_label = if count == 0 {
                        "No Items".to_owned()
                    } else {
                        format!("{count} {}", if count == 1 { "item" } else { "items" })
                    };
                    // Tuesday counts retained selected group records even when the
                    // current search hides them; group actions still use matches only.
                    let selected = group
                        .items()
                        .iter()
                        .filter(|row| self.selected.contains(&row.id))
                        .count();
                    let toggle_id = group_id.clone();
                    let heading =
                        Button::new(SharedString::from(format!("mobile-group-{group_id}")))
                            .ghost()
                            .large()
                            .w_full()
                            .icon(if folded {
                                gpui_kit::component::IconName::ChevronRight
                            } else {
                                gpui_kit::component::IconName::ChevronDown
                            })
                            .label(format!("{}   {count_label}", group.name()))
                            .accessibility_label(format!("Toggle {}", group.name()))
                            .disabled(busy)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.folded.insert(toggle_id.clone(), !folded);
                                cx.notify();
                            }));
                    column = column.child(heading);
                    if folded {
                        continue;
                    }
                    let ids: Vec<_> = matching.iter().map(|row| row.id).collect();
                    let checkbox = gpui_kit::component::checkbox::Checkbox::new(
                        SharedString::from(format!("mobile-select-group-{group_id}")),
                    )
                    .accessibility_label(format!("Select all items in {}", group.name()))
                    .checked(count > 0 && selected == count)
                    .indeterminate(selected > 0 && selected < count)
                    .disabled(busy || count == 0)
                    .on_click(cx.listener(move |view, checked, _, cx| {
                        for id in &ids {
                            if *checked {
                                view.selected.insert(*id);
                            } else {
                                view.selected.remove(id);
                            }
                        }
                        cx.notify();
                    }));
                    let mut table = div()
                        .id(SharedString::from(format!("mobile-table-{group_id}")))
                        .flex()
                        .flex_col()
                        .overflow_x_scroll()
                        .child(
                            BoardRow::new(Label::new("Item"))
                                .header()
                                .touch()
                                .leading(checkbox)
                                .cell(Label::new(status.title.clone())),
                        );
                    for row in matching {
                        let (board_id, item) = (*board, row.id);
                        let history =
                            Button::new(SharedString::from(format!("item-activity-{item}")))
                                .large()
                                .label("Activity log")
                                .accessibility_label(format!("Activity log for {}", row.title))
                                .disabled(busy)
                                .on_click(cx.listener(move |view, _, w, cx| {
                                    view.open_activity(board_id, item, w, cx)
                                }));
                        let check = gpui_kit::component::checkbox::Checkbox::new(
                            SharedString::from(format!("mobile-select-{item}")),
                        )
                        .accessibility_label(format!("Select {}", row.title))
                        .checked(self.selected.contains(&item))
                        .disabled(busy)
                        .on_click(cx.listener(
                            move |view, checked, _, cx| {
                                if *checked {
                                    view.selected.insert(item);
                                } else {
                                    view.selected.remove(&item);
                                }
                                cx.notify();
                            },
                        ));
                        table = table.child(
                            BoardRow::new(
                                div()
                                    .flex()
                                    .items_center()
                                    .w_full()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .child(Label::new(row.title.clone())),
                                    )
                                    .child(history.small()),
                            )
                            .leading(check)
                            .cell(status.cell(
                                *board,
                                row,
                                self.logging_out || self.automation_pending,
                                cx,
                            ))
                            .touch(),
                        );
                    }
                    column = column.child(table);
                }
                if !self.selected.is_empty() {
                    column = column.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(Label::new(format!("{} selected", self.selected.len())))
                            .child(
                                Button::new("mobile-clear-selection")
                                    .label("Clear selection")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.selected.clear();
                                        cx.notify();
                                    })),
                            ),
                    );
                }
                "Back to boards"
            }
            Page::Activity(activity) => {
                column = self.render_activity(activity, column, cx);
                "Back to board"
            }
            Page::Inbox(inbox) => {
                let busy = self.logging_out || inbox.pending;
                column = column.child(Label::new("Notifications").text_xl());
                column = column.child(
                    Button::new("mark-all-read")
                        .large()
                        .flex_shrink_0()
                        .label("Mark all as read")
                        .disabled(busy || inbox.unread == 0)
                        .on_click(cx.listener(|view, _, w, cx| {
                            view.inbox_action(InboxAction::ReadAll, w, cx);
                        })),
                );
                if inbox.loading {
                    column = column.child(Label::new("Loading…"));
                } else if inbox.loaded {
                    column = column.child(Label::new(if inbox.unread == 0 {
                        "You're all caught up".to_owned()
                    } else {
                        format!("{} unread", inbox.unread)
                    }));
                }
                for row in &inbox.notifications {
                    let id = row.id;
                    let board = row.board_id;
                    let unread = row.read_at.is_none();
                    let mut entry = div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .id(SharedString::from(format!("notification-{id}")));
                    if unread {
                        entry = entry.child(
                            Label::new("● Unread")
                                .id(SharedString::from(format!("notification-unread-{id}"))),
                        );
                    }
                    entry = entry
                        .child(
                            Label::new(row.title.clone())
                                .text_lg()
                                .id(SharedString::from(format!("notification-title-{id}"))),
                        )
                        .child(Label::new(row.message.clone()))
                        .child(Label::new(format!(
                            "{} — {}",
                            row.board_name, row.item_name
                        )))
                        .child(Label::new(row.created_at.clone()));
                    let actions = div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new(SharedString::from(format!("notification-read-{id}")))
                                .large()
                                .flex_shrink_0()
                                .label(if unread { "Mark read" } else { "Mark unread" })
                                .disabled(busy)
                                .on_click(cx.listener(move |view, _, w, cx| {
                                    view.inbox_action(InboxAction::Read(id, unread), w, cx);
                                })),
                        )
                        .child(
                            Button::new(SharedString::from(format!("notification-delete-{id}")))
                                .large()
                                .flex_shrink_0()
                                .label("Delete")
                                .disabled(busy)
                                .on_click(cx.listener(move |view, _, w, cx| {
                                    view.inbox_action(InboxAction::Delete(id), w, cx);
                                })),
                        )
                        .child(
                            Button::new(SharedString::from(format!("notification-open-{id}")))
                                .large()
                                .flex_shrink_0()
                                .label("Open")
                                .disabled(busy)
                                .on_click(cx.listener(move |view, _, w, cx| {
                                    view.navigate(Some(board), w, cx);
                                })),
                        );
                    column = column.child(entry.child(actions));
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
        let parent_board = match page {
            Page::Activity(activity) => Some(activity.board),
            _ => None,
        };
        column = column.child(
            Button::new("boards")
                .large()
                .label(action)
                .disabled(self.logging_out || (page.pending() && parent_board.is_none()))
                .on_click(cx.listener(move |view, _, w, cx| view.navigate(parent_board, w, cx))),
        );
        if !self.message.is_empty() {
            column = column.child(Label::new(self.message));
        }
        column = column.child(self.logout_button(cx));
        let mut page_view = div().size_full().flex().flex_col().child(
            div()
                .flex_1()
                .min_h_0()
                .child(scroller.child(column.h_auto())),
        );
        if matches!(page, Page::Selected { .. }) {
            page_view = page_view.child(self.search_dock(
                self.logging_out || self.automation_pending || page.pending(),
                cx,
            ));
        }
        page_view
    }
}
