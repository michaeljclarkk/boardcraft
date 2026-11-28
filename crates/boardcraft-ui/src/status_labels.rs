//! Retained controls for the Status label editor. The client owns all writes.
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, Hsla, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    Subscription, Task, TestSupportExt, Window,
    base::Disableable,
    component::{
        ActiveTheme, Colorize, Icon, Sizable,
        button::{Button, ButtonCustomVariant, ButtonVariants},
        color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
        input::{Input, InputEvent, InputState},
    },
    div,
    prelude::FluentBuilder,
    rems,
};
use std::time::Duration;

// Bundled Lucide shapes also render in clients whose host has no asset paths.
const PENCIL_ICON:&[u8]=br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/><path d="m15 5 4 4"/></svg>"#;
const PLUS_ICON:&[u8]=br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>"#;
const CHECK_ICON:&[u8]=br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m20 6-11 11-5-5"/></svg>"#;

pub const STATUS_LABEL_COLORS: [&str; 36] = [
    "#00c875", "#4eccc6", "#ffcb00", "#c4c4c4", "#fdab3d", "#ffadad", "#e2445c", "#ff158a",
    "#ff5ac4", "#a25ddc", "#784bd1", "#8080ff", "#66ccff", "#0086c0", "#9cd326", "#037f4c",
    "#9d50dd", "#7e3b8a", "#bb3354", "#9aadbd", "#225091", "#5559df", "#401694", "#ff7575",
    "#faa1f1", "#ff6d3b", "#cab641", "#d6b495", "#cd9282", "#757575", "#579bfc", "#74e0ef",
    "#bda8f9", "#f4d1e1", "#ffffff", "#333333",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LabelEntry {
    pub key: String,
    pub label: String,
    pub color: String,
}

#[derive(Clone, Debug)]
pub enum StatusLabelEvent {
    Edit {
        key: String,
        label: Option<String>,
        color: Option<String>,
    },
    New,
    Retry,
    Recover,
    Discard,
}

struct Field {
    entry: LabelEntry,
    input: Entity<InputState>,
    color_focus: FocusHandle,
    label: Option<String>,
    color: Option<String>,
    emitted: (Option<String>, Option<String>),
    _events: Subscription,
}

pub struct StatusLabelEditor {
    fields: Vec<Field>,
    palette: Option<String>,
    custom: Entity<ColorPickerState>,
    error: Option<String>,
    pending: bool,
    recovery_blocked: bool,
    timer: Option<Task<()>>,
    _custom_events: Subscription,
}

impl EventEmitter<StatusLabelEvent> for StatusLabelEditor {}

impl StatusLabelEditor {
    pub fn new(entries: Vec<LabelEntry>, w: &mut Window, cx: &mut Context<Self>) -> Self {
        let custom = cx.new(|cx| ColorPickerState::new(w, cx));
        let events = cx.subscribe_in(&custom, w, |this, _, event, _, cx| {
            if let ColorPickerEvent::Change(Some(color)) = event
                && let Some(key) = this.palette.clone()
            {
                this.change_color(&key, color.to_hex(), cx);
            }
        });
        let mut this = Self {
            fields: Vec::new(),
            palette: None,
            custom,
            error: None,
            pending: false,
            recovery_blocked: false,
            timer: None,
            _custom_events: events,
        };
        for entry in entries {
            this.add_field(entry, w, cx);
        }
        this
    }

    fn add_field(&mut self, entry: LabelEntry, w: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            InputState::new(w, cx)
                .placeholder("Default Label")
                .default_value(entry.label.clone())
                .validate(|value, _| value.encode_utf16().count() <= 160)
        });
        let key = entry.key.clone();
        let events = cx.subscribe_in(&input, w, move |this, input, event, w, cx| match event {
            InputEvent::Change => {
                let value = input.read(cx).value().to_string();
                if let Some(field) = this.fields.iter_mut().find(|f| f.entry.key == key) {
                    if field.label.is_none() && field.entry.label == value {
                        return;
                    }
                    field.label = Some(value);
                    this.error = None;
                    this.timer = Some(cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(450))
                            .await;
                        _ = this.update(cx, |this, cx| this.flush(cx));
                    }));
                    cx.notify();
                }
            }
            InputEvent::Blur => this.flush(cx),
            InputEvent::PressEnter { .. } => {
                this.flush(cx);
                w.blur(cx);
            }
            _ => {}
        });
        self.fields.push(Field {
            entry,
            input,
            color_focus: cx.focus_handle().tab_stop(true),
            label: None,
            color: None,
            emitted: (None, None),
            _events: events,
        });
    }

    pub fn entries(&self) -> Vec<LabelEntry> {
        self.fields
            .iter()
            .map(|f| LabelEntry {
                key: f.entry.key.clone(),
                label: f.label.clone().unwrap_or_else(|| f.entry.label.clone()),
                color: f.color.clone().unwrap_or_else(|| f.entry.color.clone()),
            })
            .collect()
    }

    /// An old acknowledgement never replaces a more recent local field edit.
    pub fn accept(&mut self, entry: LabelEntry, cx: &mut Context<Self>) {
        if let Some(field) = self.fields.iter_mut().find(|f| f.entry.key == entry.key) {
            if field.label.as_ref() == Some(&entry.label) {
                field.label = None;
                field.emitted.0 = None;
            }
            if field.color.as_ref() == Some(&entry.color) {
                field.color = None;
                field.emitted.1 = None;
            }
            field.entry = entry;
        }
        cx.notify();
    }

    /// Also refresh an untouched input after an authorized reconciliation read.
    pub fn accept_in(&mut self, entry: LabelEntry, w: &mut Window, cx: &mut Context<Self>) {
        if !self.fields.iter().any(|f| f.entry.key == entry.key) {
            self.add_field(entry, w, cx);
            cx.notify();
            return;
        }
        let key = entry.key.clone();
        self.accept(entry, cx);
        if let Some(field) = self.fields.iter().find(|f| f.entry.key == key)
            && field.label.is_none()
            && field.input.read(cx).value().as_str() != field.entry.label
        {
            let value = field.entry.label.clone();
            field
                .input
                .update(cx, |input, cx| input.set_value(value, w, cx));
        }
    }

    pub fn insert(&mut self, entry: LabelEntry, w: &mut Window, cx: &mut Context<Self>) {
        if self.fields.len() >= 250 || self.fields.iter().any(|f| f.entry.key == entry.key) {
            return;
        }
        self.add_field(entry.clone(), w, cx);
        let field = self.fields.last_mut().unwrap();
        field.label = Some(entry.label);
        field.color = Some(entry.color);
        field.input.update(cx, |input, cx| {
            input.focus(w, cx);
            input.select_all(w, cx);
        });
        self.flush(cx);
        cx.notify();
    }

    /// Restore a retained patch before the client deliberately resumes writes.
    pub fn restore_patch(
        &mut self,
        key: String,
        label: Option<String>,
        color: Option<String>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.fields.iter().any(|f| f.entry.key == key) {
            self.add_field(
                LabelEntry {
                    key: key.clone(),
                    label: label.clone().unwrap_or_default(),
                    color: color.clone().unwrap_or_else(|| "#c4c4c4".into()),
                },
                w,
                cx,
            );
        }
        if let Some(field) = self.fields.iter_mut().find(|f| f.entry.key == key) {
            field.label = label.clone();
            field.color = color.clone();
            field.emitted = (label.clone(), color);
            if let Some(value) = label {
                field.input.update(cx, |i, cx| i.set_value(value, w, cx));
            }
        }
        cx.notify();
    }

    pub fn drafts(&self) -> Vec<(String, Option<String>, Option<String>)> {
        self.fields
            .iter()
            .filter(|f| f.label.is_some() || f.color.is_some())
            .map(|f| (f.entry.key.clone(), f.label.clone(), f.color.clone()))
            .collect()
    }

    pub fn set_error(&mut self, error: Option<String>, cx: &mut Context<Self>) {
        self.error = error;
        cx.notify();
    }
    pub fn set_pending(&mut self, pending: bool, cx: &mut Context<Self>) {
        self.pending = pending;
        cx.notify();
    }
    pub fn set_recovery_blocked(&mut self, blocked: bool, cx: &mut Context<Self>) {
        self.recovery_blocked = blocked;
        cx.notify();
    }
    pub fn has_drafts(&self) -> bool {
        self.pending
            || self
                .fields
                .iter()
                .any(|f| f.label.is_some() || f.color.is_some())
    }
    pub fn focus_first(&self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(field) = self.fields.first() {
            field.input.update(cx, |i, cx| i.focus(w, cx));
        }
    }
    pub fn close_palette(&mut self, w: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(key) = self.palette.take() else {
            return false;
        };
        if let Some(field) = self.fields.iter().find(|f| f.entry.key == key) {
            field.color_focus.focus(w, cx);
        }
        cx.notify();
        true
    }
    pub fn retry_drafts(&mut self, cx: &mut Context<Self>) {
        for field in &mut self.fields {
            field.emitted = (None, None);
        }
        self.flush(cx);
    }
    fn flush(&mut self, cx: &mut Context<Self>) {
        self.timer = None;
        for field in &mut self.fields {
            let patch = (field.label.clone(), field.color.clone());
            if patch != (None, None) && patch != field.emitted {
                field.emitted = patch.clone();
                cx.emit(StatusLabelEvent::Edit {
                    key: field.entry.key.clone(),
                    label: patch.0,
                    color: patch.1,
                });
            }
        }
        cx.notify();
    }
    fn change_color(&mut self, key: &str, color: String, cx: &mut Context<Self>) {
        if let Some(field) = self.fields.iter_mut().find(|f| f.entry.key == key) {
            field.color = Some(color);
            self.error = None;
            self.palette = None;
            self.flush(cx);
        }
    }
}

impl Render for StatusLabelEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.recovery_blocked {
            return div().id("status-label-recovery").test_support().flex().flex_col().gap_2().w_full()
                .role(gpui_kit::Role::Alert)
                .child("The saved label draft could not be read. Its original bytes are kept; no labels were sent.")
                .children(self.error.clone())
                .child(Button::new("status-label-recover").label("Retry label recovery").disabled(self.pending)
                    .on_click(cx.listener(|_,_,_,cx|cx.emit(StatusLabelEvent::Recover))))
                .child(Button::new("status-label-discard-draft").label("Discard saved label draft").disabled(self.pending)
                    .on_click(cx.listener(|_,_,_,cx|cx.emit(StatusLabelEvent::Discard))))
                .into_any_element();
        }
        let mut grid = div().flex().flex_col().gap_2().w_full();
        let mut row = div().flex().gap_2().w_full();
        for (ix, field) in self.fields.iter().enumerate() {
            let key = field.entry.key.clone();
            let name = field.label.as_deref().unwrap_or(&field.entry.label);
            let color = field.color.as_deref().unwrap_or(&field.entry.color);
            let bg = Hsla::parse_hex(color).unwrap_or(cx.theme().muted_foreground);
            row = row.child(
                div()
                    .id(SharedString::from(format!(
                        "label-field-{}",
                        field.entry.key
                    )))
                    .test_support()
                    .flex_1()
                    .flex_basis(rems(0.))
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .w_full()
                            .min_w_0()
                            .h(rems(2.125))
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded_sm()
                            .px_1()
                            .child(
                                Button::new(SharedString::from(format!("label-color-{key}")))
                                    .track_focus(&field.color_focus)
                                    .small()
                                    .icon(Icon::default().data(PENCIL_ICON))
                                    .size_6()
                                    .custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(bg)
                                            .foreground(gpui_kit::white())
                                            .hover(bg.darken(0.08))
                                            .active(bg.darken(0.15)),
                                    )
                                    .bg(bg)
                                    .text_color(gpui_kit::white())
                                    .accessibility_label(format!(
                                        "Change color for {}",
                                        if name.is_empty() { "empty label" } else { name }
                                    ))
                                    .on_click(cx.listener(move |this, _, w, cx| {
                                        if this.palette.as_deref() == Some(&key) {
                                            this.close_palette(w, cx);
                                        } else {
                                            this.palette = Some(key.clone());
                                            if let Some(field) =
                                                this.fields.iter().find(|f| f.entry.key == key)
                                                && let Ok(color) = Hsla::parse_hex(
                                                    field
                                                        .color
                                                        .as_deref()
                                                        .unwrap_or(&field.entry.color),
                                                )
                                            {
                                                this.custom.update(cx, |custom, cx| {
                                                    custom.set_value(color, w, cx)
                                                });
                                            }
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                Input::new(&field.input)
                                    .id(SharedString::from(format!(
                                        "label-name-{}",
                                        field.entry.key
                                    )))
                                    .small()
                                    .appearance(false)
                                    .bordered(false)
                                    .flex_1()
                                    .flex_basis(rems(0.))
                                    .min_w_0()
                                    .aria_label(format!(
                                        "Label name: {}",
                                        if name.is_empty() { "Empty" } else { name }
                                    )),
                            ),
                    ),
            );
            if ix % 2 == 1 {
                grid = grid.child(row);
                row = div().flex().gap_2().w_full();
            }
        }
        row = row.child(
            div().flex_1().flex_basis(rems(0.)).min_w_0().child(
                Button::new("status-new-label")
                    .h(rems(2.125))
                    .w_full()
                    .min_w_0()
                    .icon(Icon::default().data(PLUS_ICON))
                    .label("New label")
                    .disabled(self.fields.len() >= 250)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(StatusLabelEvent::New))),
            ),
        );
        if self.fields.len() % 2 == 0 {
            row = row.child(div().flex_1().flex_basis(rems(0.)).min_w_0());
        }
        grid = grid.child(row);
        let mut panel = div()
            .id("status-label-editor")
            .test_support()
            .flex()
            .flex_col()
            .gap_3()
            .w_full()
            .max_h(rems(28.))
            .overflow_y_scroll()
            .child(grid);
        if let Some(key) = self.palette.clone() {
            let selected = self
                .fields
                .iter()
                .find(|f| f.entry.key == key)
                .map(|f| f.color.as_deref().unwrap_or(&f.entry.color))
                .unwrap_or("");
            let mut colors = div()
                .id("status-label-colors")
                .test_support()
                .flex()
                .flex_wrap()
                .gap_2()
                .w(rems(12.25));
            for color in STATUS_LABEL_COLORS {
                let bg = Hsla::parse_hex(color).unwrap();
                let key = key.clone();
                colors = colors.child(
                    Button::new(SharedString::from(format!("label-use-color-{color}")))
                        .w(rems(1.625))
                        .h(rems(1.625))
                        .min_w(rems(1.625))
                        .rounded_sm()
                        .custom(
                            ButtonCustomVariant::new(cx)
                                .color(bg)
                                .foreground(gpui_kit::white())
                                .hover(bg.darken(0.08))
                                .active(bg.darken(0.15)),
                        )
                        .bg(bg)
                        .text_color(gpui_kit::white())
                        .accessibility_label(format!("Use color {color}"))
                        .toggled(selected.eq_ignore_ascii_case(color))
                        .when(selected.eq_ignore_ascii_case(color), |b| {
                            b.icon(Icon::default().data(CHECK_ICON))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.change_color(&key, color.into(), cx)
                        })),
                );
            }
            panel = panel.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .w(rems(13.625))
                    .p_2()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_sm()
                    .child(colors)
                    .child(
                        ColorPicker::new(&self.custom)
                            .label("Custom")
                            .accessibility_label("Custom label color"),
                    ),
            );
        }
        if let Some(error) = &self.error {
            panel = panel.child(
                div()
                    .id("status-label-error")
                    .test_support()
                    .role(gpui_kit::Role::Alert)
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
                    .child(
                        Button::new("status-label-retry")
                            .small()
                            .label("Retry")
                            .disabled(self.pending)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(StatusLabelEvent::Retry))),
                    ),
            );
        }
        panel
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, w, cx| {
                if event.keystroke.key == "escape" && this.close_palette(w, cx) {
                    cx.stop_propagation();
                }
            }))
            .on_action(
                cx.listener(|this, _: &gpui_kit::base::actions::Cancel, w, cx| {
                    if !this.close_palette(w, cx) {
                        cx.propagate();
                    }
                }),
            )
            .into_any_element()
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AnyWindowHandle, Bounds, TestAppContext, WindowBounds, WindowOptions, px, size,
    };

    fn editor(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<StatusLabelEditor>) {
        cx.update(gpui_kit::init);
        cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(360.), px(600.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |w, cx| {
                    cx.new(|cx| {
                        StatusLabelEditor::new(
                            vec![
                                LabelEntry {
                                    key: "working".into(),
                                    label: "Working on it".into(),
                                    color: "#579bfc".into(),
                                },
                                LabelEntry {
                                    key: "empty".into(),
                                    label: "".into(),
                                    color: "#c4c4c4".into(),
                                },
                            ],
                            w,
                            cx,
                        )
                    })
                },
            )
            .unwrap()
        })
    }

    #[gpui_kit::test]
    fn label_name_is_dirty_before_debounce_and_old_ack_preserves_newer_input(
        cx: &mut TestAppContext,
    ) {
        let (w, editor) = editor(cx);
        cx.update_window(w, |_, w, cx| {
            w.render_frame(cx);
            w.click("label-name-working", cx);
            w.press("ctrl-a", cx);
            w.input("Review", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(w, |_, w, cx| {
            assert!(editor.read(cx).has_drafts());
            editor.update(cx, |e, cx| {
                e.accept_in(
                    LabelEntry {
                        key: "working".into(),
                        label: "Working on it".into(),
                        color: "#579bfc".into(),
                    },
                    w,
                    cx,
                )
            });
            w.render_frame(cx);
            assert_eq!(w.find("label-name-working").value(), Some("Review"));
            assert_eq!(editor.read(cx).drafts()[0].1.as_deref(), Some("Review"));
            editor.update(cx, |e, cx| {
                e.accept_in(
                    LabelEntry {
                        key: "working".into(),
                        label: "Review".into(),
                        color: "#579bfc".into(),
                    },
                    w,
                    cx,
                )
            });
            assert!(!editor.read(cx).has_drafts());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn color_palette_has_source_choices_and_commits_only_the_target_label(cx: &mut TestAppContext) {
        let (w, editor) = editor(cx);
        cx.update_window(w, |_, w, cx| {
            w.render_frame(cx);
            let field = w.find("label-field-working").bounds();
            let new = w.find("status-new-label").bounds();
            assert!(
                (field.size.width - new.size.width).abs() <= px(1.),
                "field {field:?} new {new:?}"
            );
            w.click("label-color-empty", cx);
            assert!(w.try_find("status-label-colors").is_some());
            for color in STATUS_LABEL_COLORS {
                assert!(w.try_find(format!("label-use-color-{color}")).is_some());
            }
            w.click("label-use-color-#e2445c", cx);
            assert!(w.try_find("status-label-colors").is_none());
            let drafts = editor.read(cx).drafts();
            assert_eq!(drafts, vec![("empty".into(), None, Some("#e2445c".into()))]);
            assert_eq!(editor.read(cx).entries()[0].color, "#579bfc");
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn accepted_remote_name_refreshes_untouched_input_and_recovered_patch_stays_dirty(
        cx: &mut TestAppContext,
    ) {
        let (w, editor) = editor(cx);
        cx.update_window(w, |_, w, cx| {
            editor.update(cx, |e, cx| {
                e.accept_in(
                    LabelEntry {
                        key: "working".into(),
                        label: "Remote".into(),
                        color: "#579bfc".into(),
                    },
                    w,
                    cx,
                )
            });
            w.render_frame(cx);
            assert_eq!(w.find("label-name-working").value(), Some("Remote"));
            assert!(!editor.read(cx).has_drafts());
            editor.update(cx, |e, cx| {
                e.restore_patch("working".into(), Some("Retained".into()), None, w, cx)
            });
            w.render_frame(cx);
            assert_eq!(w.find("label-name-working").value(), Some("Retained"));
            assert!(editor.read(cx).has_drafts());
        })
        .unwrap();
    }
}
