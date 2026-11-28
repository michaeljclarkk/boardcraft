//! Captured changes for the one item whose options menu opened this sheet.
use super::*;
use boardcraft_mobile_edit_client::ActivityEntry;
use gpui_kit::component::WindowExt;

enum ActivityRead {
    Loading,
    Failed(EditError),
    Ready(Vec<ActivityEntry>),
}

struct ActivityLog {
    client: EditClient,
    board: Uuid,
    item: Uuid,
    search: Entity<InputState>,
    _search_events: Subscription,
    offset: usize,
    read: ActivityRead,
}

impl Connected {
    pub(super) fn open_item_activity(
        &mut self,
        item: Uuid,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Navigation(nav) = &self.screen else {
            return;
        };
        let Page::Items(board, table) = &nav.page else {
            return;
        };
        let Some(trigger) = table.archive_triggers.get(&item) else {
            return;
        };
        let Some(row) = table
            .headings
            .iter()
            .flat_map(|h| &h.rows)
            .find(|r| r.read(cx).edit.identity().1.as_uuid() == item)
        else {
            return;
        };
        let title = row.read(cx).edit.accepted().to_owned();
        let log = cx.new(|cx| ActivityLog::new(nav.client.clone(), *board, item, w, cx));
        log.update(cx, |log, cx| log.load(cx));
        // The popup item disappears. Kit's sheet should restore the row menu instead.
        trigger.focus(w, cx);
        w.open_sheet(cx, move |sheet, _, _| {
            sheet
                .size(rems(32.))
                .max_w_full()
                .title(Label::new(title.clone()))
                .child(log.clone())
        });
    }
}

impl ActivityLog {
    fn new(
        client: EditClient,
        board: Uuid,
        item: Uuid,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(w, cx).placeholder("Person, change or value"));
        let events = cx.subscribe(&search, |this, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                this.offset = 0;
                cx.notify();
            }
        });
        Self {
            client,
            board,
            item,
            search,
            _search_events: events,
            offset: 0,
            read: ActivityRead::Loading,
        }
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.read = ActivityRead::Loading;
        let client = self.client.clone();
        let (board, item) = (self.board, self.item);
        let work = cx
            .background_executor()
            .spawn(async move { client.item_activity(board, item) });
        // Only the opening sheet owns this entity. Late reads cannot populate a reopened item.
        cx.spawn(async move |log, cx| {
            let result = work.await;
            _ = log.update(cx, |log, cx| {
                log.read = match result {
                    Ok(entries) => ActivityRead::Ready(entries),
                    Err(error) => ActivityRead::Failed(error),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn entries(&self, entries: &[ActivityEntry], cx: &mut Context<Self>) -> Div {
        const PAGE_SIZE: usize = 50;
        let term = self.search.read(cx).value().trim().to_lowercase();
        let mut count = 0;
        let mut list = div()
            .id("activity-entries")
            .role(Role::List)
            .flex()
            .flex_col();
        for entry in entries {
            if !term.is_empty()
                && ![&entry.actor_name, &entry.field, &entry.before, &entry.after]
                    .iter()
                    .any(|s| s.to_lowercase().contains(&term))
            {
                continue;
            }
            if (self.offset..self.offset + PAGE_SIZE).contains(&count) {
                list = list.child(activity_entry(entry, cx));
            }
            count += 1;
        }
        let mut body = div().flex().flex_col().gap_3().child(
            Label::new(format!("{count} changes"))
                .text_sm()
                .text_color(cx.theme().muted_foreground),
        );
        if count == 0 {
            return body
                .child(Label::new(if term.is_empty() {
                    "No captured activity for this item"
                } else {
                    "No matching activity"
                }))
                .child(
                    Label::new(if term.is_empty() {
                        "Changes appear here after an item is edited."
                    } else {
                        "Try another person, change or value."
                    })
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
                );
        }
        body = body.child(list);
        if count > PAGE_SIZE {
            let end = (self.offset + PAGE_SIZE).min(count);
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        Button::new("activity-newer")
                            .small()
                            .label("Newer changes")
                            .disabled(self.offset == 0)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.offset = this.offset.saturating_sub(PAGE_SIZE);
                                cx.notify();
                            })),
                    )
                    .child(Label::new(format!("{}–{end} of {count}", self.offset + 1)))
                    .child(
                        Button::new("activity-older")
                            .small()
                            .label("Older changes")
                            .disabled(end == count)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.offset = end;
                                cx.notify();
                            })),
                    ),
            );
        }
        body
    }
}

fn activity_entry(entry: &ActivityEntry, cx: &App) -> impl IntoElement {
    let value = |s: &str| {
        Label::new(if s.is_empty() { "Empty" } else { s }.to_owned())
            .bg(cx.theme().muted)
            .px_2()
            .py_1()
            .min_w_0()
    };
    let time = entry
        .occurred_at
        .strip_suffix('Z')
        .map(|iso| format!("{} UTC", iso.replace('T', " ")))
        .unwrap_or_else(|| entry.occurred_at.clone());
    div()
        .id(SharedString::from(format!(
            "activity-{}-{}",
            entry.revision, entry.field
        )))
        .role(Role::ListItem)
        .flex()
        .gap_3()
        .py_4()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .size_8()
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(cx.theme().accent)
                .text_color(cx.theme().accent_foreground)
                .child(
                    entry
                        .actor_name
                        .chars()
                        .next()
                        .map(|c| c.to_uppercase().to_string())
                        .unwrap_or_default(),
                ),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(Label::new(entry.actor_name.clone()))
                        .child(
                            Label::new(time)
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        ),
                )
                .child(Label::new(format!(
                    "Changed {} · Revision {}",
                    entry.field, entry.revision
                )))
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap_2()
                        .child(value(&entry.before).id("previous-value"))
                        .child("→")
                        .child(value(&entry.after).id("new-value")),
                ),
        )
}

impl Render for ActivityLog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = div()
            .id("activity-log")
            .role(Role::Region)
            .aria_label("Activity Log")
            .flex()
            .flex_col()
            .gap_3()
            .py_3()
            .child(Label::new("Activity Log").text_lg())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(Label::new("Search activity").text_sm())
                    .child(
                        Input::new(&self.search)
                            .id("activity-search")
                            .aria_label("Search activity"),
                    ),
            );
        match &self.read {
            ActivityRead::Loading => body.child(
                div()
                    .id("activity-loading")
                    .role(Role::Status)
                    .child(Label::new("Loading captured activity…")),
            ),
            ActivityRead::Failed(error) => body
                .child(
                    div()
                        .id("activity-error")
                        .role(Role::Alert)
                        .child(Label::new(match error {
                            EditError::SessionRejected => "Session expired. Sign in again.",
                            EditError::PermissionDenied => {
                                "You do not have access to this activity."
                            }
                            EditError::Missing => "This item is unavailable.",
                            _ => "Activity could not be loaded.",
                        })),
                )
                .child(
                    Button::new("retry-activity")
                        .label("Retry activity")
                        .on_click(cx.listener(|this, _, _, cx| this.load(cx))),
                ),
            ActivityRead::Ready(entries) => body.child(self.entries(entries, cx)),
        }
    }
}

#[cfg(all(test, feature = "native-smoke"))]
mod tests;
