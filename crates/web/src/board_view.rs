use boardcraft_mobile_edit_client::{EditError, NameEdit, SavedItem};
use gpui_kit::StatefulInteractiveElement;
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Render, Role, Styled, Subscription, Window,
    base::Disableable,
    component::{
        button::Button,
        input::{Input, InputEvent, InputState},
    },
    div,
};
use uuid::Uuid;

#[derive(Clone)]
pub(super) struct Rename {
    item: Uuid,
    raw: String,
    revision: i64,
    retry: bool,
}
impl Rename {
    pub(super) fn item(&self) -> Uuid {
        self.item
    }
    pub(super) fn edit(&self) -> NameEdit {
        NameEdit {
            title: self.raw.clone(),
            revision: self.revision,
        }
    }
    pub(super) fn is_retry(&self) -> bool {
        self.retry
    }
}
impl EventEmitter<Rename> for NameRow {}

enum Phase {
    Display,
    Editing,
    Pending(Rename),
    Rejected(Rename, &'static str),
}
pub(super) struct NameRow {
    accepted: SavedItem,
    editable: bool,
    phase: Phase,
    input: Entity<InputState>,
    focus: FocusHandle,
    _events: Subscription,
}
impl NameRow {
    pub(super) fn sync(&mut self, saved: &SavedItem, editable: bool, cx: &mut Context<Self>) {
        self.editable = editable;
        if matches!(self.phase, Phase::Display) {
            self.accepted = saved.clone();
        }
        cx.notify();
    }
    pub(super) fn new(
        accepted: SavedItem,
        editable: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(w, cx));
        let events = cx.subscribe_in(&input, w, |row, _, event, w, cx| match event {
            InputEvent::PressEnter { .. } => {
                row.submit(true, w, cx);
                cx.stop_propagation();
            }
            InputEvent::Blur => row.submit(false, w, cx),
            _ => (),
        });
        Self {
            accepted,
            editable,
            phase: Phase::Display,
            input,
            focus: cx.focus_handle().tab_stop(true),
            _events: events,
        }
    }
    fn open(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if !self.editable || matches!(self.phase, Phase::Pending(_) | Phase::Editing) {
            return;
        }
        self.phase = Phase::Editing;
        self.input.update(cx, |input, cx| {
            input.set_value(self.accepted.title.clone(), w, cx);
            input.focus(w, cx);
        });
        cx.notify();
    }
    fn submit(&mut self, return_focus: bool, w: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.phase, Phase::Editing) {
            return;
        }
        let raw = self.input.read(cx).value().to_string();
        if raw == self.accepted.title {
            self.phase = Phase::Display;
        } else {
            let intent = Rename {
                item: self.accepted.id,
                raw,
                revision: self.accepted.revision,
                retry: false,
            };
            self.phase = Phase::Pending(intent.clone());
            cx.emit(intent);
        }
        if return_focus {
            self.focus.focus(w, cx);
        }
        cx.notify();
    }
    pub(super) fn finished(
        &mut self,
        result: Result<SavedItem, EditError>,
        read: Option<SavedItem>,
        cx: &mut Context<Self>,
    ) {
        if let Some(read) = read {
            self.accepted = read;
        }
        match result {
            Ok(row) => {
                self.accepted = row;
                self.phase = Phase::Display;
            }
            Err(error) => {
                let reason = match error {
                    EditError::InvalidDraft => "Name rejected",
                    EditError::Conflict => "Name changed elsewhere",
                    EditError::PermissionDenied => "Editing is no longer allowed",
                    EditError::Missing => "Item unavailable",
                    _ => "Could not save name",
                };
                if let Phase::Pending(mut request) =
                    std::mem::replace(&mut self.phase, Phase::Display)
                {
                    request.retry = true;
                    self.phase = Phase::Rejected(request, reason);
                }
            }
        }
        cx.notify();
    }
}
impl Render for NameRow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.accepted.id;
        let mut row = div().flex().flex_col().gap_2();
        if matches!(self.phase, Phase::Editing) {
            row = row
                .capture_action(
                    cx.listener(|row, _: &gpui_kit::component::input::Enter, w, cx| {
                        row.submit(true, w, cx);
                        cx.stop_propagation();
                    }),
                )
                .capture_action(cx.listener(
                    |row, _: &gpui_kit::component::input::Escape, w, cx| {
                        row.phase = Phase::Display;
                        row.focus.focus(w, cx);
                        cx.stop_propagation();
                        cx.notify();
                    },
                ))
                // A horizontal table row gives an input no intrinsic text width.
                // Reserve a usable editor cell instead of shrinking to padding.
                .child(
                    Input::new(&self.input)
                        .w_64()
                        .aria_label(format!("Item name {id}")),
                );
        } else {
            let label = if let Phase::Pending(request) = &self.phase {
                request.raw.clone()
            } else {
                self.accepted.title.clone()
            };
            row = row
                .on_key_down(cx.listener(|row, event: &gpui_kit::KeyDownEvent, w, cx| {
                    if event.keystroke.key == "f2" {
                        row.open(w, cx);
                        cx.stop_propagation();
                    }
                }))
                .child(
                    Button::new(gpui_kit::SharedString::from(format!("item-{id}")))
                        .track_focus(&self.focus)
                        .label(label)
                        .accessibility_id(id.to_string())
                        .disabled(!self.editable || matches!(self.phase, Phase::Pending(_)))
                        .on_click(cx.listener(|row, event: &gpui_kit::ClickEvent, w, cx| {
                            row.focus.focus(w, cx);
                            if event.click_count() == 2 || event.is_keyboard() {
                                row.open(w, cx);
                            }
                        })),
                );
        }
        match &self.phase {
            Phase::Pending(_) => {
                row = row.child(
                    div()
                        .id("name-status")
                        .role(Role::Status)
                        .aria_label("Saving…")
                        .child("Saving…"),
                )
            }
            Phase::Rejected(_, reason) => {
                row = row
                    .child(
                        div()
                            .id("name-status")
                            .role(Role::Status)
                            .aria_label(*reason)
                            .child(*reason),
                    )
                    .child(
                        Button::new("retry-name")
                            .label("Retry change")
                            .on_click(cx.listener(|row, _, _, cx| {
                                if let Phase::Rejected(request, _) =
                                    std::mem::replace(&mut row.phase, Phase::Display)
                                {
                                    row.phase = Phase::Pending(request.clone());
                                    cx.emit(request);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("dismiss-name")
                            .label("Dismiss")
                            .on_click(cx.listener(|row, _, _, cx| {
                                row.phase = Phase::Display;
                                cx.notify();
                            })),
                    )
            }
            _ => (),
        }
        row
    }
}
