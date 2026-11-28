//! A board-owned label draft survives popover dismissal attempts and uncertain
//! replies. Only a deliberate Retry sends a failed patch again.
use super::*;
use boardcraft_mobile_edit_client::{StatusLabelEdit, StatusLabels};
use boardcraft_ui::{LabelEntry, STATUS_LABEL_COLORS, StatusLabelEditor, StatusLabelEvent};
use std::collections::VecDeque;
mod draft;
use draft::Journal;

pub(super) struct LabelControls {
    pub(super) can_manage_labels: bool,
    pub(super) label_actor: Uuid,
    pub(super) picker_open: Option<String>,
    pub(super) labels: Option<Box<LabelDraft>>,
}

impl LabelControls {
    pub(super) fn new(can_manage_labels: bool, label_actor: Uuid) -> Self {
        Self {
            can_manage_labels,
            label_actor,
            picker_open: None,
            labels: None,
        }
    }
}

pub(super) struct LabelDraft {
    editor: Entity<StatusLabelEditor>,
    // Worker-returned snapshots keep UI subscription ownership in the entity map.
    _subscriptions: Entity<(Subscription, Subscription)>,
    queue: VecDeque<(String, StatusLabelEdit)>,
    pub(super) pending: bool,
    pub(super) dirty: bool,
    blocked: bool,
    review_required: bool,
    close_requested: bool,
    journal: Journal,
    recovery_message: Option<String>,
}

impl PickerTarget {
    fn board_column(self) -> (Uuid, Uuid) {
        match self {
            Self::Cell(target) => (target.board, target.column),
            Self::Selection(target) => (target.board, target.column),
        }
    }
}

fn entry(choice: &StatusChoice) -> LabelEntry {
    LabelEntry {
        key: choice.key.clone(),
        label: choice.label.clone(),
        color: choice.color.clone(),
    }
}

fn merge(queue: &mut VecDeque<(String, StatusLabelEdit)>, key: String, edit: StatusLabelEdit) {
    if let Some((_, old)) = queue.iter_mut().find(|(id, _)| id == &key) {
        if edit.label.is_some() {
            old.label = edit.label;
        }
        if edit.color.is_some() {
            old.color = edit.color;
        }
    } else {
        queue.push_back((key, edit));
    }
}

fn matches(choice: &StatusChoice, patch: &StatusLabelEdit) -> bool {
    patch
        .label
        .as_ref()
        .is_none_or(|name| choice.label == *name)
        && patch
            .color
            .as_ref()
            .is_none_or(|color| choice.color == *color)
}

fn remove_confirmed(queue: &mut VecDeque<(String, StatusLabelEdit)>, choice: &StatusChoice) {
    for (_, patch) in queue.iter_mut().filter(|(key, _)| key == &choice.key) {
        if patch.label.as_ref() == Some(&choice.label) {
            patch.label = None;
        }
        if patch.color.as_ref() == Some(&choice.color) {
            patch.color = None;
        }
    }
    queue.retain(|(_, patch)| patch.label.is_some() || patch.color.is_some());
}

enum LabelResult {
    Saved(StatusChoice),
    Reviewed(Result<StatusLabels, EditError>),
    Failed(EditError, Result<StatusLabels, EditError>),
}

impl BoardView {
    fn status_for_labels(&mut self, target: PickerTarget) -> Option<&mut StatusColumn> {
        let (board, column) = target.board_column();
        if let PickerTarget::Selection(target) = target
            && target.visit != self.navigation_visit
        {
            return None;
        }
        match &mut self.navigation {
            Some(Page::Selected {
                board: current,
                status,
                ..
            }) if *current == board && status.id == column => Some(status),
            _ => None,
        }
    }

    pub(super) fn status_picker_open(
        &mut self,
        id: &str,
        target: PickerTarget,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(status) = self.status_for_labels(target) else {
            return;
        };
        if open {
            status.label_controls.picker_open = Some(id.to_owned());
        } else if let Some(draft) = &mut status.label_controls.labels {
            if draft
                .editor
                .update(cx, |editor, cx| editor.close_palette(window, cx))
            {
                cx.notify();
                return;
            }
            draft.dirty = draft.journal.unreadable || draft.editor.read(cx).has_drafts();
            if draft.pending || draft.dirty {
                draft.close_requested = true;
                cx.notify();
                return;
            }
            status.label_controls.picker_open = None;
            status.label_controls.labels = None;
        } else {
            status.label_controls.picker_open = None;
        }
        cx.notify();
    }

    pub(super) fn open_status_labels(
        &mut self,
        target: PickerTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editor.is_some() || self.logging_out {
            return;
        }
        let Some(connection) = self.connection else {
            return;
        };
        let Some(status) = self.status_for_labels(target) else {
            return;
        };
        if !status.label_controls.can_manage_labels
            || status.pending.is_some()
            || status.selection_pending
        {
            return;
        }
        if let Some(draft) = &status.label_controls.labels {
            draft
                .editor
                .update(cx, |editor, cx| editor.focus_first(window, cx));
            return;
        }
        let entries = status.choices.iter().map(entry).collect();
        let actor = status.label_controls.label_actor;
        let (board, column) = target.board_column();
        let choices = status.choices.clone();
        let (mut journal, recovered) = Journal::open(connection.port, actor, board, column);
        let editor = cx.new(|cx| StatusLabelEditor::new(entries, window, cx));
        editor.update(cx, |editor, cx| {
            for (key, patch) in &recovered {
                editor.restore_patch(
                    key.clone(),
                    patch.label.clone(),
                    patch.color.clone(),
                    window,
                    cx,
                );
            }
            for choice in choices.iter() {
                editor.accept_in(entry(choice), window, cx);
            }
        });
        let mut recovered_queue = VecDeque::from(recovered);
        for choice in choices.iter() {
            remove_confirmed(&mut recovered_queue, choice);
        }
        if !journal.unreadable {
            journal.retain(
                editor
                    .read(cx)
                    .drafts()
                    .into_iter()
                    .map(|(key, label, color)| (key, StatusLabelEdit { label, color }))
                    .collect(),
            );
        }
        let events = cx.subscribe_in(&editor, window, move |view, editor, event, window, cx| {
            match event {
                StatusLabelEvent::Edit { key, label, color } => {
                    let Some(status) = view.status_for_labels(target) else {
                        return;
                    };
                    let Some(draft) = &mut status.label_controls.labels else {
                        return;
                    };
                    merge(
                        &mut draft.queue,
                        key.clone(),
                        StatusLabelEdit {
                            label: label.clone(),
                            color: color.clone(),
                        },
                    );
                    draft.dirty = true;
                    if draft.blocked && !draft.review_required && status.label_controls.can_manage_labels {
                        // A corrected field is a new deliberate edit, as in the source.
                        draft.blocked = false;
                    }
                    if draft.review_required {
                        editor.update(cx, |editor, cx| {
                            editor.set_error(Some("Could not confirm the label change. Retry checks current labels before another save.".into()), cx);
                        });
                    } else if !status.label_controls.can_manage_labels {
                        editor.update(cx, |editor, cx| {
                            editor.set_error(Some("Only an Owner can edit labels. Your draft is kept.".into()), cx);
                        });
                    }
                    if !draft.blocked {
                        view.flush_status_labels(target, window, cx);
                    }
                }
                StatusLabelEvent::New => {
                    let count = editor.read(cx).entries().len();
                    if count >= 250 {
                        return;
                    }
                    let key = format!("l-{}", Uuid::new_v4());
                    let color = STATUS_LABEL_COLORS[count % STATUS_LABEL_COLORS.len()].to_owned();
                    editor.update(cx, |editor, cx| {
                        editor.insert(
                            LabelEntry {
                                key,
                                label: "New label".into(),
                                color,
                            },
                            window,
                            cx,
                        )
                    });
                }
                StatusLabelEvent::Retry => {
                    let Some(status) = view.status_for_labels(target) else {
                        return;
                    };
                    let Some(draft) = &mut status.label_controls.labels else {
                        return;
                    };
                    if draft.pending {
                        return;
                    }
                    draft.queue = editor.read(cx).drafts().into_iter()
                        .map(|(key,label,color)| (key,StatusLabelEdit{label,color})).collect();
                    draft.blocked = false;
                    draft.review_required = true;
                    view.flush_status_labels(target, window, cx);
                }
                StatusLabelEvent::Recover => view.recover_status_labels(target, false, window, cx),
                StatusLabelEvent::Discard => view.recover_status_labels(target, true, window, cx),
            }
            cx.notify();
        });
        let changes = cx.observe(&editor, move |view, editor, cx| {
            if let Some(status) = view.status_for_labels(target)
                && let Some(draft) = &mut status.label_controls.labels
            {
                draft.dirty = draft.journal.unreadable || editor.read(cx).has_drafts();
                let patches = editor.read(cx).drafts().into_iter().map(|(key,label,color)| (key,StatusLabelEdit{label,color})).collect();
                let was_error = draft.journal.storage_error;
                if !draft.journal.unreadable && !draft.journal.retain(patches) && !was_error {
                    editor.update(cx, |editor,cx| editor.set_error(Some("Storage unavailable. Keep this editor open until your changes finish.".into()),cx));
                }
                cx.notify();
            }
        });
        let subscriptions = cx.new(|_| (events, changes));
        let Some(status) = self.status_for_labels(target) else {
            return;
        };
        status.label_controls.labels = Some(Box::new(LabelDraft {
            editor: editor.clone(),
            _subscriptions: subscriptions,
            queue: recovered_queue,
            pending: false,
            dirty: journal.unreadable || editor.read(cx).has_drafts(),
            blocked: journal.unreadable,
            review_required: false,
            close_requested: false,
            journal,
            recovery_message: None,
        }));
        editor.update(cx, |editor, cx| editor.focus_first(window, cx));
        self.flush_status_labels(target, window, cx);
        cx.notify();
    }

    pub(super) fn status_labels_panel(
        &mut self,
        target: PickerTarget,
        return_focus: FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let status = self.status_for_labels(target)?;
        let draft = status.label_controls.labels.as_ref()?;
        let editor = draft.editor.clone();
        let blocked = draft.pending || draft.dirty;
        let width = (window.viewport_size().width / window.rem_size() - 3.).clamp(16., 21.);
        if draft.journal.unreadable {
            return Some(div().flex().flex_col().gap_2().w(rems(width))
                .child(Label::new("The saved label draft could not be read. Its original bytes are kept; no labels were sent."))
                .children(draft.recovery_message.clone().map(Label::new))
                .child(Button::new("mobile-label-recovery").large().label("Retry label recovery")
                    .on_click(cx.listener(move |view,_,window,cx| view.recover_status_labels(target,false,window,cx))))
                .child(Button::new("mobile-label-discard-draft").large().label("Discard saved label draft")
                    .on_click(cx.listener(move |view,_,window,cx| view.recover_status_labels(target,true,window,cx))))
                .on_key_down(|event:&KeyDownEvent,_,cx| { if event.keystroke.key == "escape" { cx.stop_propagation(); } })
                .into_any_element());
        }
        Some(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .w(rems(width))
                .child(editor)
                .child(
                    Button::new("mobile-status-back-to-labels")
                        .large()
                        .ghost()
                        .w_full()
                        .label("Back to labels")
                        .disabled(blocked)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            if let Some(status) = view.status_for_labels(target)
                                && status
                                    .label_controls
                                    .labels
                                    .as_ref()
                                    .is_some_and(|draft| !draft.pending && !draft.dirty)
                            {
                                status.label_controls.labels = None;
                                return_focus.focus(window, cx);
                                cx.notify();
                            }
                        })),
                )
                .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        let id = view
                            .status_for_labels(target)
                            .and_then(|status| status.label_controls.picker_open.clone());
                        if let Some(id) = id {
                            view.status_picker_open(&id, target, false, window, cx);
                        }
                        cx.stop_propagation();
                    }
                }))
                .into_any_element(),
        )
    }

    fn flush_status_labels(
        &mut self,
        target: PickerTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let visit = self.navigation_visit;
        let Some(status) = self.status_for_labels(target) else {
            return;
        };
        let Some(draft) = &mut status.label_controls.labels else {
            return;
        };
        if draft.pending || draft.blocked {
            return;
        }
        let read_only = draft.review_required;
        let request = if read_only {
            None
        } else {
            draft.queue.pop_front()
        };
        if request.is_none() && !read_only {
            return;
        }
        draft.pending = true;
        let patches = draft
            .editor
            .read(cx)
            .drafts()
            .into_iter()
            .map(|(key, label, color)| (key, StatusLabelEdit { label, color }))
            .collect();
        draft.journal.retain(patches);
        draft.editor.update(cx, |editor, cx| {
            editor.set_pending(true, cx);
            editor.set_error(None, cx);
        });
        let (board, column) = target.board_column();
        let sent = request.as_ref().map(|(key, patch)| {
            (
                key.clone(),
                StatusLabelEdit {
                    label: patch.label.clone(),
                    color: patch.color.clone(),
                },
            )
        });
        let work = cx.background_executor().spawn(async move {
            if let Some((key, patch)) = &sent {
                match client.save_status_label(board, column, key, patch) {
                    Ok(saved) => LabelResult::Saved(saved),
                    Err(error) => {
                        LabelResult::Failed(error, client.read_status_labels(board, column))
                    }
                }
            } else {
                LabelResult::Reviewed(client.read_status_labels(board, column))
            }
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = work.await;
            _ = view.update_in(cx, |view, window, cx| {
                if view.navigation_visit != visit { return; }
                let Some(status) = view.status_for_labels(target) else { return; };
                let Some(draft) = &mut status.label_controls.labels else { return; };
                draft.pending = false;
                draft.editor.update(cx, |editor, cx| editor.set_pending(false, cx));
                let mut keep_going = true;
                let mut accepted = None;
                let mut failure = None;
                match result {
                    LabelResult::Saved(choice) => accepted = Some(choice),
                    LabelResult::Reviewed(loaded) => {
                        failure = Some((None, loaded));
                        keep_going = false;
                    }
                    LabelResult::Failed(error, loaded) => failure = Some((Some(error), loaded)),
                }
                if let Some((error, loaded)) = failure {
                    match loaded {
                        Ok(loaded) => {
                            status.label_controls.can_manage_labels = loaded.can_edit;
                            status.choices = loaded.choices.into();
                            draft.review_required = !loaded.can_edit;
                            draft.editor.update(cx, |editor, cx| {
                                for choice in status.choices.iter() {
                                    editor.accept_in(entry(choice), window, cx);
                                }
                            });
                            for choice in status.choices.iter() { remove_confirmed(&mut draft.queue, choice); }
                            let known = request.as_ref().and_then(|(key, patch)| {
                                status.choices.iter().find(|choice| choice.key == *key && matches(choice, patch)).cloned()
                            });
                            if error.is_none() && loaded.can_edit {
                                // This read belongs to a deliberate Retry. Continue that
                                // same action with only fields that still differ.
                                draft.blocked = false;
                                keep_going = true;
                            } else if error.is_some() && known.is_some() {
                                accepted = known;
                            } else {
                                draft.blocked = true;
                                keep_going = false;
                                if let Some((key, patch)) = request {
                                    let mut retry = VecDeque::from([(key, patch)]);
                                    for (key, patch) in draft.queue.drain(..) { merge(&mut retry, key, patch); }
                                    draft.queue = retry;
                                }
                                let text = if !loaded.can_edit {
                                    "Only an Owner can edit labels. Your draft is kept."
                                } else if matches!(error, Some(EditError::InvalidDraft)) {
                                    "Use a label name of at most 160 characters or choose a colour."
                                } else {
                                    "Current labels reloaded. Review your draft and choose Retry."
                                };
                                draft.editor.update(cx, |editor, cx| editor.set_error(Some(text.into()), cx));
                            }
                        }
                        Err(_) => {
                            draft.review_required = true;
                            draft.blocked = true;
                            keep_going = false;
                            if let Some((key, patch)) = request {
                                let mut retry = VecDeque::from([(key, patch)]);
                                for (key, patch) in draft.queue.drain(..) { merge(&mut retry, key, patch); }
                                draft.queue = retry;
                            }
                            draft.editor.update(cx, |editor, cx| editor.set_error(Some(
                                "Could not confirm the label change. Retry checks current labels before another save.".into()
                            ), cx));
                        }
                    }
                }
                if let Some(choice) = accepted {
                    remove_confirmed(&mut draft.queue, &choice);
                    let mut choices = status.choices.to_vec();
                    if let Some(current) = choices.iter_mut().find(|c| c.key == choice.key) {
                        *current = choice.clone();
                    } else { choices.push(choice.clone()); }
                    status.choices = choices.into();
                    draft.editor.update(cx, |editor, cx| editor.accept_in(entry(&choice), window, cx));
                }
                draft.dirty = draft.editor.read(cx).has_drafts();
                draft.journal.retain(draft.editor.read(cx).drafts().into_iter()
                    .map(|(key,label,color)| (key,StatusLabelEdit{label,color})).collect());
                let close = draft.close_requested && !draft.dirty && !draft.pending;
                if close { status.label_controls.picker_open = None; status.label_controls.labels = None; }
                if keep_going && !close { view.flush_status_labels(target, window, cx); }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    fn recover_status_labels(
        &mut self,
        target: PickerTarget,
        discard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(status) = self.status_for_labels(target) else {
            return;
        };
        let Some(draft) = &mut status.label_controls.labels else {
            return;
        };
        if discard {
            if !draft.journal.discard() {
                draft.recovery_message = Some("The saved label draft changed or could not be discarded. Retry recovery before discarding it.".into());
                cx.notify();
                return;
            }
            draft.dirty = false;
            draft.blocked = false;
            draft.review_required = false;
            draft.recovery_message = None;
            draft
                .editor
                .update(cx, |editor, cx| editor.set_error(None, cx));
        } else if let Some(patches) = draft.journal.recover() {
            draft.recovery_message = None;
            draft.queue = patches.clone().into();
            draft.editor.update(cx, |editor, cx| {
                for (key, patch) in patches {
                    editor.restore_patch(key, patch.label, patch.color, window, cx);
                }
                for choice in status.choices.iter() {
                    editor.accept_in(entry(choice), window, cx);
                }
                editor.set_error(None, cx);
            });
            for choice in status.choices.iter() {
                remove_confirmed(&mut draft.queue, choice);
            }
            draft.dirty = draft.editor.read(cx).has_drafts();
            draft.blocked = false;
            draft.review_required = false;
        } else {
            draft.recovery_message = Some("The saved label draft is still unreadable.".into());
        }
        self.flush_status_labels(target, window, cx);
        cx.notify();
    }
}
