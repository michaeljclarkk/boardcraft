use super::*;
use boardcraft_mobile_edit_client::ActivityEntry;
use gpui_kit::component::{ActiveTheme, avatar::Avatar};

const PAGE_SIZE: usize = 50;

pub(crate) struct ItemActivity {
    pub(super) board: Uuid,
    item: Uuid,
    title: String,
    load: Load,
    search: Entity<InputState>,
    offset: usize,
}

enum Load {
    Loading(Uuid),
    Ready(Vec<ActivityEntry>),
    Failed(&'static str),
}

impl ItemActivity {
    pub(super) fn loading(&self) -> bool {
        matches!(self.load, Load::Loading(_))
    }
}

impl BoardView {
    pub(super) fn open_activity(
        &mut self,
        board: Uuid,
        item: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(client), Some(conn)) = (self.client.clone(), self.connection) else {
            return;
        };
        if self.logging_out || self.login.is_some() || self.editor.is_some() {
            return;
        }
        let (title, search) = match &self.navigation {
            Some(Page::Selected {
                board: selected,
                rows,
                status,
                filter,
                ..
            }) if board == *selected && !status.is_pending() => {
                let Some(row) = rows
                    .iter()
                    .find(|row| row.id == item && view_filter_matches(filter, &row.title))
                else {
                    return;
                };
                let search =
                    cx.new(|cx| InputState::new(window, cx).placeholder("Person, change or value"));
                cx.subscribe(&search, |view, input, event, cx| {
                    if matches!(event, InputEvent::Change)
                        && let Some(Page::Activity(activity)) = &mut view.navigation
                        && activity.search == input
                    {
                        activity.offset = 0;
                        cx.notify();
                    }
                })
                .detach();
                (row.title.clone(), search)
            }
            Some(Page::Activity(activity))
                if activity.board == board && activity.item == item && !activity.loading() =>
            {
                (activity.title.clone(), activity.search.clone())
            }
            _ => return,
        };
        let request = Uuid::new_v4();
        self.navigation = Some(Page::Activity(ItemActivity {
            board,
            item,
            title,
            search,
            load: Load::Loading(request),
            offset: 0,
        }));
        self.message = "";
        window.focus(&self.navigation_focus, cx);
        let identifier = Login::identifier(window, cx);
        let work = cx
            .background_executor()
            .spawn(async move { client.item_activity(board, item) });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            _ = view.update(cx, |view, cx| {
                // Leaving the page (including session replacement) discards the request.
                // Reopening the same item starts a distinct request even at the same origin.
                let Some(Page::Activity(current)) = &mut view.navigation else {
                    return;
                };
                if !matches!(current.load, Load::Loading(id) if id == request) {
                    return;
                }
                match result {
                    Ok(entries) => current.load = Load::Ready(entries),
                    Err(EditError::SessionRejected) => view.forget_session(conn, identifier, cx),
                    Err(EditError::PermissionDenied | EditError::Missing) => {
                        view.navigation = Some(Page::Denied)
                    }
                    Err(EditError::AccessUnavailable) => {
                        current.load = Load::Failed("Access check unavailable. Try again.")
                    }
                    Err(_) => current.load = Load::Failed("Activity could not be loaded."),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn render_activity(
        &self,
        activity: &ItemActivity,
        mut column: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        let (board, item) = (activity.board, activity.item);
        column = column
            .child(Label::new("Activity log").text_xl())
            .child(Label::new(activity.title.clone()))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(Label::new("Search activity"))
                    .child(
                        Input::new(&activity.search)
                            .id("activity-search")
                            .large()
                            .disabled(self.logging_out),
                    ),
            );
        let entries = match &activity.load {
            Load::Loading(_) => return column.child(Label::new("Loading captured activity…")),
            Load::Failed(message) => {
                return column.child(Label::new(*message)).child(
                    Button::new("retry-activity")
                        .large()
                        .label("Retry activity")
                        .disabled(self.logging_out)
                        .on_click(cx.listener(move |view, _, w, cx| {
                            view.open_activity(board, item, w, cx)
                        })),
                );
            }
            Load::Ready(entries) => entries,
        };
        let query = activity.search.read(cx).value().trim().to_lowercase();
        let filtered: Vec<_> = entries
            .iter()
            .filter(|entry| {
                query.is_empty()
                    || [&entry.actor_name, &entry.field, &entry.before, &entry.after]
                        .into_iter()
                        .any(|s| s.to_lowercase().contains(&query))
            })
            .collect();
        let total = filtered.len();
        column = column.child(Label::new(format!("{total} changes")));
        if total == 0 {
            let (title, hint) = if query.is_empty() {
                (
                    "No captured activity for this item",
                    "The imported source history may cover only a limited period.",
                )
            } else {
                (
                    "No matching activity",
                    "Try another person, change or value.",
                )
            };
            return column.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(Label::new(title).text_lg())
                    .child(Label::new(hint)),
            );
        }
        let offset = activity.offset.min((total - 1) / PAGE_SIZE * PAGE_SIZE);
        for entry in filtered.into_iter().skip(offset).take(PAGE_SIZE) {
            let details = div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(Label::new(entry.actor_name.clone()).text_lg())
                .child(
                    Label::new(entry.occurred_at.clone()).text_color(cx.theme().muted_foreground),
                )
                .child(Label::new(format!(
                    "{} · Revision {}",
                    entry.field, entry.revision
                )))
                .child(Label::new(format!(
                    "{} → {}",
                    display_value(&entry.before),
                    display_value(&entry.after)
                )));
            column = column.child(
                div()
                    .flex()
                    .gap_2()
                    .items_start()
                    .child(Avatar::new().name(entry.actor_name.clone()))
                    .child(details),
            );
        }
        if total > PAGE_SIZE {
            let change_page = |id, label, next, disabled| {
                Button::new(id)
                    .large()
                    .label(label)
                    .disabled(self.logging_out || disabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if let Some(Page::Activity(current)) = &mut view.navigation
                            && current.board == board
                            && current.item == item
                        {
                            current.offset = next;
                            cx.notify();
                        }
                    }))
            };
            column = column.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(change_page(
                        "newer-activity",
                        "Newer changes",
                        offset.saturating_sub(PAGE_SIZE),
                        offset == 0,
                    ))
                    .child(Label::new(format!(
                        "{}–{} of {total}",
                        offset + 1,
                        (offset + PAGE_SIZE).min(total)
                    )))
                    .child(change_page(
                        "older-activity",
                        "Older changes",
                        offset + PAGE_SIZE,
                        offset + PAGE_SIZE >= total,
                    )),
            );
        }
        column
    }
}

fn display_value(value: &str) -> &str {
    if value.is_empty() { "Empty" } else { value }
}
