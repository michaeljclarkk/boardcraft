//! Rename recovery belongs to the board visit, even when the item's group is folded.
use super::{AccessLost, Rename, Table, key};
use crate::session;
use boardcraft_mobile_edit_client::{EditError, SavedItem};
use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, Role, StatefulInteractiveElement,
    Styled, Window,
    base::Disableable,
    component::{
        ActiveTheme, Sizable,
        button::{Button, ButtonVariants},
    },
    div,
};
use uuid::Uuid;

pub(super) struct FailedRename {
    request: Rename,
    reason: String,
    reading: bool,
}

fn reason(error: &EditError) -> &'static str {
    match error {
        EditError::InvalidDraft => "An item needs a name of 1–255 characters.",
        EditError::Conflict => {
            "This field changed elsewhere. Reload the latest version, review your retained draft, then retry."
        }
        EditError::Missing => "This item is no longer available on this board.",
        EditError::PermissionDenied => "Editing is no longer allowed.",
        EditError::SessionRejected => "Your session has ended.",
        EditError::AccessUnavailable => "Access could not be checked. Try again.",
        EditError::HostFault => "The server could not complete the request.",
        EditError::Transport => {
            "The server response could not be confirmed. Check your connection and try again."
        }
    }
}

impl Table {
    pub(super) fn rename(&mut self, request: Rename, w: &mut Window, cx: &mut Context<Self>) {
        let item = request.item();
        self.failed_renames.retain(|f| f.request.item() != item);
        let binding = self.binding.clone();
        let board = self.board;
        let executor = cx.background_executor().clone();
        self.saves.retain(|task| !task.is_ready());
        self.saves.push(cx.spawn_in(w, async move |table, cx| {
            let result = session::rename(&binding, board, item, &request.edit(), &executor).await;
            let _ = table.update(cx, |table, cx| table.rename_finished(request, result, cx));
        }));
        cx.notify();
    }

    pub(super) fn rename_finished(
        &mut self,
        request: Rename,
        result: Result<SavedItem, EditError>,
        cx: &mut Context<Self>,
    ) {
        let item = request.item();
        let result = result.and_then(|saved| {
            if saved.id == item {
                Ok(saved)
            } else {
                Err(EditError::Transport)
            }
        });
        if let Some(row) = self.rows.get(&item) {
            row.update(cx, |row, cx| row.finished(result.as_ref().ok(), cx));
        }
        if let Err(error) = result {
            self.failed_renames.push(FailedRename {
                request,
                reason: reason(&error).into(),
                reading: false,
            });
            if matches!(
                error,
                EditError::SessionRejected | EditError::PermissionDenied
            ) {
                cx.emit(AccessLost);
            }
        }
        cx.notify();
    }

    fn retry_rename(&mut self, item: Uuid, w: &mut Window, cx: &mut Context<Self>) {
        let Some(failure) = self
            .failed_renames
            .iter_mut()
            .find(|f| f.request.item() == item && !f.reading)
        else {
            return;
        };
        if !self.read.is_editable()
            || self
                .rows
                .get(&item)
                .is_some_and(|row| !row.update(cx, |r, cx| r.begin_retry_read(cx)))
        {
            return;
        }
        failure.reading = true;
        let binding = self.binding.clone();
        let board = self.board;
        let executor = cx.background_executor().clone();
        self.saves.retain(|task| !task.is_ready());
        self.saves.push(cx.spawn_in(w, async move |table, cx| {
            let result = session::name_for_retry(&binding, board, item, &executor).await;
            let _ = table.update_in(cx, |table, w, cx| {
                let Some(failure) = table
                    .failed_renames
                    .iter_mut()
                    .find(|f| f.request.item() == item && f.reading)
                else {
                    return;
                };
                match result {
                    Ok(current) => {
                        let request = failure.request.at_current_item(current);
                        if let Some(row) = table.rows.get(&item) {
                            row.update(cx, |row, cx| row.project_retry(&request, cx));
                        }
                        table.rename(request, w, cx);
                    }
                    Err(error) => {
                        failure.reading = false;
                        failure.reason = format!("Could not reload the item. {}", reason(&error));
                        if let Some(row) = table.rows.get(&item) {
                            row.update(cx, |row, cx| row.finished(None, cx));
                        }
                        if matches!(
                            error,
                            EditError::SessionRejected | EditError::PermissionDenied
                        ) {
                            cx.emit(AccessLost);
                        }
                        cx.notify();
                    }
                }
            });
        }));
        cx.notify();
    }

    pub(super) fn rename_notice(
        &self,
        failure: &FailedRename,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let item = failure.request.item();
        let message = format!(
            "{}: this change could not be saved. {}",
            failure.request.accepted_title(),
            failure.reason
        );
        div()
            .id(key(format!("rename-notice-{item}")))
            .role(Role::Alert)
            .aria_label(message.clone())
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .p_3()
            .min_w_0()
            .text_sm()
            .border_1()
            .border_color(cx.theme().danger)
            .bg(cx.theme().danger.opacity(0.08))
            .child(message)
            .child(
                Button::new(key(format!("retry-name-{item}")))
                    .ghost()
                    .small()
                    .label(if failure.reading {
                        "Reading item…"
                    } else {
                        "Retry change"
                    })
                    .disabled(failure.reading || !self.read.is_editable())
                    .on_click(cx.listener(move |t, _, w, cx| t.retry_rename(item, w, cx))),
            )
            .child(
                Button::new(key(format!("dismiss-name-{item}")))
                    .ghost()
                    .small()
                    .label("Dismiss")
                    .disabled(failure.reading)
                    .on_click(cx.listener(move |t, _, _, cx| {
                        t.failed_renames
                            .retain(|f| f.request.item() != item || f.reading);
                        cx.notify();
                    })),
            )
    }
}
