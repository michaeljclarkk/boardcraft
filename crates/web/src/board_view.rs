use boardcraft_mobile_edit_client::{NameEdit, SavedItem};
use gpui_kit::StatefulInteractiveElement;
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Render, Role, Styled, Subscription, Window,
    base::Disableable,
    component::{
        Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
    },
    div,
};
use uuid::Uuid;

#[derive(Clone)]
pub(super) struct Rename {
    accepted: SavedItem,
    raw: String,
}
impl Rename {
    pub(super) fn item(&self) -> Uuid {
        self.accepted.id
    }
    pub(super) fn accepted_title(&self) -> &str {
        &self.accepted.title
    }
    pub(super) fn edit(&self) -> NameEdit {
        NameEdit {
            title: self.raw.clone(),
            revision: self.accepted.revision,
        }
    }
    pub(super) fn at_current_item(&self, current: SavedItem) -> Self {
        Self {
            accepted: current,
            raw: self.raw.clone(),
        }
    }
}
impl EventEmitter<Rename> for NameRow {}

enum Phase {
    Display,
    Editing(SavedItem),
    ReadingForRetry,
    Pending(String),
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
    pub(super) fn accepted_title(&self) -> &str {
        &self.accepted.title
    }
    pub(super) fn sync(&mut self, saved: &SavedItem, editable: bool, cx: &mut Context<Self>) {
        self.editable = editable;
        self.observe(saved);
        cx.notify();
    }
    fn observe(&mut self, saved: &SavedItem) {
        if saved.revision >= self.accepted.revision {
            self.accepted = saved.clone();
        }
    }
    pub(super) fn has_request(&self) -> bool {
        matches!(self.phase, Phase::ReadingForRetry | Phase::Pending(_))
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
        if !self.editable || !matches!(self.phase, Phase::Display) {
            return;
        }
        self.phase = Phase::Editing(self.accepted.clone());
        self.input.update(cx, |input, cx| {
            input.set_value(self.accepted.title.clone(), w, cx);
            input.focus(w, cx);
        });
        cx.notify();
    }
    fn submit(&mut self, return_focus: bool, w: &mut Window, cx: &mut Context<Self>) {
        let Phase::Editing(entry) = &self.phase else {
            return;
        };
        let raw = self.input.read(cx).value().to_string();
        if raw == entry.title {
            self.phase = Phase::Display;
        } else {
            let intent = Rename {
                accepted: entry.clone(),
                raw,
            };
            self.phase = Phase::Pending(intent.raw.clone());
            cx.emit(intent);
        }
        if return_focus {
            self.focus.focus(w, cx);
        }
        cx.notify();
    }
    pub(super) fn begin_retry_read(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.editable || !matches!(self.phase, Phase::Display) {
            return false;
        }
        self.phase = Phase::ReadingForRetry;
        cx.notify();
        true
    }
    pub(super) fn project_retry(&mut self, request: &Rename, cx: &mut Context<Self>) {
        self.observe(&request.accepted);
        self.phase = Phase::Pending(request.raw.clone());
        cx.notify();
    }
    pub(super) fn finished(&mut self, saved: Option<&SavedItem>, cx: &mut Context<Self>) {
        self.phase = Phase::Display;
        if let Some(saved) = saved {
            self.observe(saved);
        }
        cx.notify();
    }
}
impl Render for NameRow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.accepted.id;
        let mut row = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .items_start()
            .justify_center()
            .gap_1();
        if matches!(self.phase, Phase::Editing(_)) {
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
                .child(
                    Input::new(&self.input)
                        .id(gpui_kit::SharedString::from(format!("edit-name-{id}")))
                        .small()
                        .w_full()
                        .accessibility_id(format!("edit-name-{id}"))
                        .aria_label("Edit item name"),
                );
        } else {
            let label = if let Phase::Pending(raw) = &self.phase {
                raw.clone()
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
                        .ghost()
                        .max_w_full()
                        .track_focus(&self.focus)
                        .label(label)
                        .accessibility_id(id.to_string())
                        .disabled(!self.editable || !matches!(self.phase, Phase::Display))
                        .on_click(cx.listener(|row, event: &gpui_kit::ClickEvent, w, cx| {
                            row.focus.focus(w, cx);
                            if event.click_count() == 2
                                || event.is_keyboard()
                                || w.last_input_was_touch()
                            {
                                row.open(w, cx);
                            }
                        })),
                );
        }
        if matches!(self.phase, Phase::Pending(_)) {
            row = row.child(
                div()
                    .id("name-status")
                    .role(Role::Status)
                    .aria_label("Saving…")
                    .child("Saving…"),
            );
        }
        row
    }
}
