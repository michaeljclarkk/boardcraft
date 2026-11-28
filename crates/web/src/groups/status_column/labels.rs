//! Inline label configuration keeps drafts in the board visit and submits once.
use super::*;
use boardcraft_mobile_edit_client::{StatusLabelEdit, StatusLabels};
use boardcraft_ui::{LabelEntry, STATUS_LABEL_COLORS, StatusLabelEditor, StatusLabelEvent};
use gpui_kit::base::actions::Cancel;

pub(super) struct LabelManagement {
    pub(super) item: Option<Uuid>,
    pub(super) close_requested: bool,
    column: Uuid,
    opening: bool,
    saving: bool,
    can_edit: bool,
    blocked_save: bool,
    error: Option<String>,
    editor: Option<Entity<StatusLabelEditor>>,
    queued: Vec<(String, StatusLabelEdit)>,
    popup: Option<WeakEntity<PopoverState>>,
    request: Option<Task<()>>,
    events: Option<Subscription>,
    observer: Option<Subscription>,
    #[cfg(target_family = "wasm")]
    draft_key: Option<String>,
    #[cfg(target_family = "wasm")]
    retained: Option<String>,
    recovered: Vec<(String, StatusLabelEdit)>,
    unreadable: bool,
    storage_failed: bool,
    #[cfg(target_family = "wasm")]
    draft_writing: bool,
    #[cfg(target_family = "wasm")]
    draft_write: Option<Task<()>>,
}

impl LabelManagement {
    pub(super) fn new() -> Self {
        Self {
            item: None,
            close_requested: false,
            column: Uuid::nil(),
            opening: false,
            saving: false,
            can_edit: false,
            blocked_save: false,
            error: None,
            editor: None,
            queued: Vec::new(),
            popup: None,
            request: None,
            events: None,
            observer: None,
            #[cfg(target_family = "wasm")]
            draft_key: None,
            #[cfg(target_family = "wasm")]
            retained: None,
            recovered: Vec::new(),
            unreadable: false,
            storage_failed: false,
            #[cfg(target_family = "wasm")]
            draft_writing: false,
            #[cfg(target_family = "wasm")]
            draft_write: None,
        }
    }

    pub(super) fn blocked(&self, cx: &App) -> bool {
        self.opening
            || self.saving
            || self.unreadable
            || self.storage_failed
            || !self.queued.is_empty()
            || self
                .editor
                .as_ref()
                .is_some_and(|editor| editor.read(cx).has_drafts())
    }

    pub(super) fn focus(&self, window: &mut Window, cx: &mut App) {
        if let Some(editor) = &self.editor {
            editor.update(cx, |editor, cx| editor.focus_first(window, cx));
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

fn satisfies(choice: &StatusChoice, edit: &StatusLabelEdit) -> bool {
    edit.label
        .as_ref()
        .is_none_or(|label| &choice.label == label)
        && edit
            .color
            .as_ref()
            .is_none_or(|color| choice.color.eq_ignore_ascii_case(color))
}

fn recover(snapshot: Option<&str>) -> Result<Vec<(String, StatusLabelEdit)>, ()> {
    let Some(snapshot) = snapshot else {
        return Ok(Vec::new());
    };
    let entries: Vec<(String, StatusLabelEdit)> = serde_json::from_str(snapshot).map_err(|_| ())?;
    if entries.len() > 250 {
        return Err(());
    }
    let mut seen = std::collections::HashSet::new();
    for (key, patch) in &entries {
        if key.is_empty()
            || key.len() > 128
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || !seen.insert(key)
            || (patch.label.is_none() && patch.color.is_none())
            || patch
                .label
                .as_ref()
                .is_some_and(|label| label.encode_utf16().count() > 160)
            || patch.color.as_ref().is_some_and(|color| {
                color.len() != 7
                    || !color.starts_with('#')
                    || !color[1..].bytes().all(|b| b.is_ascii_hexdigit())
            })
        {
            return Err(());
        }
    }
    Ok(entries)
}

fn merge(queue: &mut Vec<(String, StatusLabelEdit)>, key: String, patch: StatusLabelEdit) {
    if let Some((_, current)) = queue.iter_mut().find(|(id, _)| id == &key) {
        if patch.label.is_some() {
            current.label = patch.label;
        }
        if patch.color.is_some() {
            current.color = patch.color;
        }
    } else {
        queue.push((key, patch));
    }
}

impl Table {
    pub(super) fn status_labels_content(
        &mut self,
        item: Uuid,
        popup: WeakEntity<PopoverState>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.status.labels.item != Some(item) {
            return None;
        }
        self.status.labels.popup = Some(popup);
        let mut content = div()
            .id("status-label-management")
            .flex()
            .flex_col()
            .gap_2()
            .w(rems(21.));
        if self.status.labels.opening {
            content = content.child("Loading labels…");
        } else if self.status.labels.unreadable || self.status.labels.storage_failed {
            content=content.child(if self.status.labels.unreadable {
                "The saved label draft could not be read. Its original bytes are kept; no labels were sent."
            } else { "Could not read saved labels. No changes were sent. Retry label recovery." })
            .child(Button::new("status-label-recovery").label("Retry label recovery").on_click(cx.listener(|table,_,w,cx|table.read_status_labels(w,cx))))
            .children(self.status.labels.unreadable.then(||Button::new("status-label-discard-draft").label("Discard saved label draft").on_click(cx.listener(|table,_,w,cx|table.discard_status_label_draft(w,cx)))))
            .children(self.status.labels.error.clone());
        } else if let Some(editor) = &self.status.labels.editor {
            content = content.child(editor.clone());
        } else if let Some(error) = &self.status.labels.error {
            content = content.child(error.clone()).child(
                Button::new("status-label-read-retry")
                    .label("Retry")
                    .on_click(cx.listener(|table, _, w, cx| {
                        table.read_status_labels(w, cx);
                    })),
            );
        }
        let blocked = self.status.labels.blocked(cx);
        Some(
            content
                .child(
                    div()
                        .w_full()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .pt_2()
                        .child(
                            Button::new("status-back-to-labels")
                                .ghost()
                                .w_full()
                                .label("Back to labels")
                                .disabled(blocked)
                                .on_click(
                                    cx.listener(|table, _, w, cx| table.leave_status_labels(w, cx)),
                                ),
                        ),
                )
                .on_action(cx.listener(|table, _: &Cancel, w, cx| {
                    if let Some(editor) = &table.status.labels.editor {
                        let closed = editor.update(cx, |editor, cx| editor.close_palette(w, cx));
                        if closed {
                            cx.stop_propagation();
                            return;
                        }
                    }
                    if table.status.labels.blocked(cx) {
                        table.status.labels.close_requested = true;
                        cx.stop_propagation();
                    }
                }))
                .into_any_element(),
        )
    }

    pub(super) fn open_status_labels(
        &mut self,
        item: Uuid,
        column: Uuid,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() || !self.read.is_manageable() {
            return;
        }
        self.status.labels.item = Some(item);
        self.status.labels.column = column;
        self.status.labels.close_requested = false;
        self.read_status_labels(w, cx);
    }

    fn leave_status_labels(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.status.labels.blocked(cx) {
            return;
        }
        self.status.labels.item = None;
        self.status.labels.close_requested = false;
        if let Some(item) = self
            .status
            .labels
            .popup
            .as_ref()
            .and_then(WeakEntity::upgrade)
        {
            let focus = self
                .status
                .cells
                .values()
                .find(|focus| focus.open.get())
                .map(|focus| focus.selected.clone());
            if let Some(focus) = focus {
                focus.focus(w, cx);
            }
            item.update(cx, |_, cx| cx.notify());
        }
        cx.notify();
    }

    fn read_status_labels(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.status.labels.opening || self.status.labels.saving {
            return;
        }
        self.status.labels.opening = true;
        self.status.labels.error = None;
        let generation = self.status.generation;
        let column = self.status.labels.column;
        let (binding, board, executor) = (
            self.binding.clone(),
            self.board,
            cx.background_executor().clone(),
        );
        self.status.labels.request = Some(cx.spawn_in(w, async move |table, cx| {
            let result = session::read_status_labels(&binding, board, column, &executor).await;
            #[cfg(target_family = "wasm")]
            let draft = if let Ok(read) = &result {
                let key = format!("status-labels:{}:{board}:{column}", read.actor_id);
                let result = crate::binding::read_label_draft(&key).await;
                Some((key, result))
            } else {
                None
            };
            let _ = table.update_in(cx, |table, w, cx| {
                #[cfg(target_family = "wasm")]
                if generation == table.status.generation
                    && let Some((key, draft)) = draft
                {
                    table.status.labels.draft_key = Some(key);
                    match draft {
                        Ok(raw) => {
                            table.status.labels.storage_failed = false;
                            table.status.labels.retained = raw;
                            match recover(table.status.labels.retained.as_deref()) {
                                Ok(patches) => {
                                    table.status.labels.unreadable = false;
                                    table.status.labels.recovered = patches;
                                }
                                Err(()) => {
                                    table.status.labels.unreadable = true;
                                    table.status.labels.recovered.clear();
                                }
                            }
                        }
                        Err(()) => table.status.labels.storage_failed = true,
                    }
                }
                table.finish_status_labels_read(generation, result, w, cx)
            });
        }));
        cx.notify();
    }

    fn finish_status_labels_read(
        &mut self,
        generation: Uuid,
        result: Result<StatusLabels, EditError>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if generation != self.status.generation || self.status.labels.item.is_none() {
            return;
        }
        self.status.labels.opening = false;
        match result {
            Ok(read) if read.column_id == self.status.labels.column => {
                self.status.labels.can_edit = read.can_edit;
                self.accept_status_label_choices(&read.choices, cx);
                if self.status.labels.unreadable || self.status.labels.storage_failed {
                    cx.notify();
                    return;
                }
                self.status.labels.blocked_save = !read.can_edit;
                if !read.can_edit {
                    self.status.labels.blocked_save = true;
                    self.status.labels.error =
                        Some("Only an Owner can edit labels. Your draft is kept.".into());
                } else if self.status.labels.editor.is_none() {
                    let entries = read.choices.iter().map(entry).collect();
                    let editor = cx.new(|cx| StatusLabelEditor::new(entries, w, cx));
                    self.status.labels.events =
                        Some(cx.subscribe_in(&editor, w, |table, _, event, w, cx| {
                            table.label_event(event, w, cx)
                        }));
                    self.status.labels.observer =
                        Some(cx.observe_in(&editor, w, |table, _, w, cx| {
                            table.persist_status_label_draft(w, cx);
                            cx.notify();
                        }));
                    for (key, patch) in std::mem::take(&mut self.status.labels.recovered) {
                        editor.update(cx, |editor, cx| {
                            editor.restore_patch(key, patch.label, patch.color, w, cx)
                        });
                    }
                    editor.update(cx, |editor, cx| editor.focus_first(w, cx));
                    self.status.labels.editor = Some(editor.clone());
                    if editor.read(cx).has_drafts() {
                        cx.defer_in(w, move |_, _, cx| {
                            editor.update(cx, |editor, cx| editor.retry_drafts(cx))
                        });
                    }
                } else {
                    self.status.labels.blocked_save = false;
                    self.status.labels.queued.clear();
                    if let Some(editor) = &self.status.labels.editor {
                        for choice in &read.choices {
                            editor.update(cx, |editor, cx| editor.accept_in(entry(choice), w, cx));
                        }
                        let editor = editor.clone();
                        cx.defer_in(w, move |_, _, cx| {
                            editor.update(cx, |editor, cx| editor.retry_drafts(cx));
                        });
                    }
                }
            }
            _ => {
                self.status.labels.error =
                    Some("Could not load labels. Retry to check current labels and access.".into())
            }
        }
        if let Some(editor) = &self.status.labels.editor {
            let error = self.status.labels.error.clone();
            editor.update(cx, |editor, cx| editor.set_error(error, cx));
        }
        self.close_status_labels_when_saved(w, cx);
        cx.notify();
    }

    fn accept_status_label_choices(&mut self, choices: &[StatusChoice], cx: &mut Context<Self>) {
        if let Some(table) = &mut self.status.accepted {
            table.choices = choices.to_vec();
        }
        cx.notify();
    }

    fn label_event(&mut self, event: &StatusLabelEvent, w: &mut Window, cx: &mut Context<Self>) {
        if (self.status.labels.unreadable || self.status.labels.storage_failed)
            && !matches!(event, StatusLabelEvent::Recover | StatusLabelEvent::Discard)
        {
            return;
        }
        match event {
            StatusLabelEvent::Edit { key, label, color } => {
                if !self.status.labels.can_edit {
                    return;
                }
                merge(
                    &mut self.status.labels.queued,
                    key.clone(),
                    StatusLabelEdit {
                        label: label.clone(),
                        color: color.clone(),
                    },
                );
                if !self.status.labels.blocked_save {
                    self.save_next_status_label(w, cx);
                }
            }
            StatusLabelEvent::Retry | StatusLabelEvent::Recover => self.read_status_labels(w, cx),
            StatusLabelEvent::Discard => self.discard_status_label_draft(w, cx),
            StatusLabelEvent::New => {
                if !self.status.labels.can_edit || self.status.labels.blocked_save {
                    return;
                }
                if let Some(editor) = &self.status.labels.editor {
                    let count = editor.read(cx).entries().len();
                    if count >= 250 {
                        return;
                    }
                    let editor = editor.clone();
                    let entry = LabelEntry {
                        key: format!("l-{}", Uuid::new_v4()),
                        label: "New label".into(),
                        color: STATUS_LABEL_COLORS[count % STATUS_LABEL_COLORS.len()].into(),
                    };
                    cx.defer_in(w, move |_, w, cx| {
                        editor.update(cx, |editor, cx| editor.insert(entry, w, cx));
                    });
                }
            }
        }
        cx.notify();
    }

    fn save_next_status_label(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.status.labels.saving
            || self.status.labels.blocked_save
            || self.status.labels.queued.is_empty()
            || !self.status.labels.can_edit
        {
            return;
        }
        let (key, patch) = self.status.labels.queued.remove(0);
        self.status.labels.saving = true;
        if let Some(editor) = &self.status.labels.editor {
            editor.update(cx, |editor, cx| {
                editor.set_pending(true, cx);
                editor.set_error(None, cx);
            });
        }
        let generation = self.status.generation;
        let column = self.status.labels.column;
        let (binding, board, executor) = (
            self.binding.clone(),
            self.board,
            cx.background_executor().clone(),
        );
        self.status.labels.request = Some(cx.spawn_in(w, async move |table, cx| {
            let result =
                session::save_status_label(&binding, board, column, &key, &patch, &executor).await;
            let reread = if result.is_err() {
                Some(session::read_status_labels(&binding, board, column, &executor).await)
            } else {
                None
            };
            let _ = table.update_in(cx, |table, w, cx| {
                table.finish_status_label_save(generation, key, patch, result, reread, w, cx)
            });
        }));
        cx.notify();
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_status_label_save(
        &mut self,
        generation: Uuid,
        key: String,
        patch: StatusLabelEdit,
        result: Result<StatusChoice, EditError>,
        reread: Option<Result<StatusLabels, EditError>>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if generation != self.status.generation {
            return;
        }
        self.status.labels.saving = false;
        let accepted = match result {
            Ok(choice) => Some(choice),
            Err(_) => match reread {
                Some(Ok(read)) if read.column_id == self.status.labels.column => {
                    self.status.labels.can_edit = read.can_edit;
                    self.accept_status_label_choices(&read.choices, cx);
                    read.choices
                        .into_iter()
                        .find(|choice| choice.key == key && satisfies(choice, &patch))
                }
                _ => None,
            },
        };
        if let Some(choice) = accepted {
            if let Some(table) = &mut self.status.accepted {
                if let Some(current) = table
                    .choices
                    .iter_mut()
                    .find(|current| current.key == choice.key)
                {
                    *current = choice.clone();
                } else {
                    table.choices.push(choice.clone());
                }
            }
            if let Some(editor) = &self.status.labels.editor {
                editor.update(cx, |editor, cx| editor.accept_in(entry(&choice), w, cx));
            }
            self.status.labels.error = None;
            self.save_next_status_label(w, cx);
        } else {
            let newer = std::mem::take(&mut self.status.labels.queued);
            merge(&mut self.status.labels.queued, key, patch);
            for (key, patch) in newer {
                merge(&mut self.status.labels.queued, key, patch);
            }
            self.status.labels.blocked_save = true;
            self.status.labels.error = Some(if self.status.labels.can_edit {
                "Label change not confirmed. Your draft is kept. Retry to review current labels before sending it again.".into()
            } else {
                "Only an Owner can edit labels. Your draft is kept.".into()
            });
        }
        if let Some(editor) = &self.status.labels.editor {
            let pending = self.status.labels.saving;
            let error = self.status.labels.error.clone();
            editor.update(cx, |editor, cx| {
                editor.set_pending(pending, cx);
                editor.set_error(error, cx);
            });
        }
        self.close_status_labels_when_saved(w, cx);
        cx.notify();
    }

    fn close_status_labels_when_saved(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.status.labels.close_requested && !self.status.labels.blocked(cx) {
            self.status.labels.item = None;
            self.status.labels.close_requested = false;
            for focus in self.status.cells.values() {
                focus.open.set(false);
            }
            if let Some(popup) = &self.status.labels.popup {
                let _ = popup.update(cx, |popup, cx| popup.dismiss(w, cx));
            }
        }
    }

    fn persist_status_label_draft(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        #[cfg(target_family = "wasm")]
        {
            if self.status.labels.draft_writing
                || self.status.labels.unreadable
                || self.status.labels.storage_failed
            {
                return;
            }
            let Some(key) = self.status.labels.draft_key.clone() else {
                return;
            };
            let Some(editor) = &self.status.labels.editor else {
                return;
            };
            let patches: Vec<_> = editor
                .read(cx)
                .drafts()
                .into_iter()
                .map(|(key, label, color)| (key, StatusLabelEdit { label, color }))
                .collect();
            let wanted = if patches.is_empty() {
                None
            } else {
                serde_json::to_string(&patches).ok()
            };
            if wanted == self.status.labels.retained {
                return;
            }
            let expected = self.status.labels.retained.clone();
            let generation = self.status.generation;
            self.status.labels.draft_writing = true;
            self.status.labels.draft_write=Some(cx.spawn_in(w,async move |table,cx|{
                let result=crate::binding::update_label_draft(&key,expected,wanted.clone()).await;
                let _=table.update_in(cx,|table,w,cx|{
                    if generation!=table.status.generation || table.status.labels.draft_key.as_deref()!=Some(&key){return;}
                    table.status.labels.draft_writing=false;
                    if result==Ok(true){
                        table.status.labels.retained=wanted;
                        table.persist_status_label_draft(w,cx);
                    }else{
                        table.status.labels.storage_failed=true;
                        table.status.labels.blocked_save=true;
                        table.status.labels.error=Some("The label draft could not be retained or changed in another tab. Keep this editor open and retry label recovery.".into());
                    }
                    cx.notify();
                });
            }));
        }
        #[cfg(not(target_family = "wasm"))]
        {
            let _ = (w, cx);
        }
    }

    fn discard_status_label_draft(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        #[cfg(target_family = "wasm")]
        {
            let Some(key) = self.status.labels.draft_key.clone() else {
                return;
            };
            let expected = self.status.labels.retained.clone();
            let generation = self.status.generation;
            self.status.labels.opening = true;
            self.status.labels.request=Some(cx.spawn_in(w,async move |table,cx|{
                let result=crate::binding::update_label_draft(&key,expected,None).await;
                let _=table.update_in(cx,|table,w,cx|{
                    if generation!=table.status.generation{return;}
                    table.status.labels.opening=false;
                    if result==Ok(true) {
                        table.status.labels.retained=None;
                        table.status.labels.unreadable=false;
                        table.read_status_labels(w,cx);
                    } else {
                        table.status.labels.error=Some("The saved label draft changed or could not be discarded. Retry label recovery.".into());
                    }
                    cx.notify();
                });
            }));
        }
        #[cfg(not(target_family = "wasm"))]
        {
            let _ = (w, cx);
        }
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use crate::groups::tests::visit_titles;
    use gpui_kit::{
        TestAppContext,
        test::{TestEventExt, TestQueryExt, TestWindowExt},
    };

    #[gpui_kit::test]
    fn odd_status_choice_keeps_equal_tracks_in_cell_and_bulk_pickers(cx: &mut TestAppContext) {
        let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
        cx.update(|cx| cx.set_reduce_motion(true));
        cx.update_window(handle, |_, w, cx| {
            let mut accepted = palette();
            accepted.choices.push(StatusChoice {
                key: "l-review".into(),
                label: "Needs review".into(),
                color: "#ffcb00".into(),
            });
            table.update(cx, |table, cx| table.status.accept(accepted, cx));
            w.render_frame(cx);
            w.click(format!("status-{}", Uuid::from_u128(59)), cx);
            let first = w.find("status-choice-working").bounds();
            let last = w.find("status-choice-l-review").bounds();
            assert_eq!(first.size.width, last.size.width);
            assert_eq!(first.left(), last.left());
            w.press("escape", cx);
            w.click(format!("check-{}", Uuid::from_u128(59)), cx);
            w.click("bulk-status-column", cx);
            let status = w.find_by_label("Status").bounds().center();
            w.pointer_down(status, MouseButton::Left, cx);
            w.pointer_up(status, MouseButton::Left, cx);
            w.click("bulk-status-label", cx);
            let first = w.find("bulk-status-choice-working").bounds();
            let last = w.find("bulk-status-choice-l-review").bounds();
            assert_eq!(first.size.width, last.size.width);
            assert_eq!(first.left(), last.left());
        })
        .unwrap();
    }

    #[test]
    fn retained_label_patches_reject_corruption_without_replacing_the_bytes() {
        let raw =
            r##"[["working",{"label":"  Waiting  ","color":"#ffcb00"}],["empty",{"label":""}]]"##;
        let recovered = recover(Some(raw)).unwrap();
        assert_eq!(recovered[0].1.label.as_deref(), Some("  Waiting  "));
        assert_eq!(recovered[1].1.label.as_deref(), Some(""));
        for raw in [
            "broken",
            r##"[["done",{}]]"##,
            r##"[["done",{"label":"x"}],["done",{"color":"#123456"}]]"##,
            r##"[["done",{"color":"red"}]]"##,
            r##"[["done",{"revision":1}]]"##,
        ] {
            assert!(recover(Some(raw)).is_err(), "{raw}");
        }
    }

    fn palette() -> StatusTable {
        StatusTable {
            column_id: Uuid::from_u128(500),
            title: "Status".into(),
            choices: [
                ("working", "Working on it", "#579bfc"),
                ("done", "Done", "#00c875"),
                ("stuck", "Stuck", "#fdab3d"),
                ("empty", "Unassigned", "#a25ddc"),
            ]
            .into_iter()
            .map(|(key, label, color)| StatusChoice {
                key: key.into(),
                label: label.into(),
                color: color.into(),
            })
            .collect(),
            cells: [59, 61]
                .into_iter()
                .map(|id| StatusCell {
                    item_id: Uuid::from_u128(id),
                    revision: 7,
                    value: None,
                })
                .collect(),
        }
    }

    fn finish_open(table: &Entity<Table>, w: &mut Window, cx: &mut App) {
        table.update(cx, |table, cx| {
            // The platform UI requested its real transport; deliver the host's reply.
            table.status.labels.request = None;
            let generation = table.status.generation;
            table.finish_status_labels_read(
                generation,
                Ok(StatusLabels {
                    actor_id: Uuid::from_u128(700),
                    column_id: Uuid::from_u128(500),
                    choices: palette().choices,
                    can_edit: true,
                }),
                w,
                cx,
            );
        });
        w.render_frame(cx);
    }

    #[gpui_kit::test]
    fn configured_empty_uses_null_and_owner_edit_labels_returns_to_picker(cx: &mut TestAppContext) {
        let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
        cx.update(|cx| cx.set_reduce_motion(true));
        cx.update_window(handle, |_, w, cx| {
            table.update(cx, |table, cx| table.status.accept(palette(), cx));
            w.render_frame(cx);
            let trigger = format!("status-{}", Uuid::from_u128(59));
            assert_eq!(
                w.find(trigger.clone()).label(),
                Some("Status for Ink: Unassigned")
            );
            w.click(trigger.clone(), cx);
            assert_eq!(w.find("status-empty").checked(), Some(true));
            assert!(w.try_find("status-choice-empty").is_none());
            w.click("status-empty", cx);
            assert!(!table.read(cx).pending);
            assert_eq!(
                table.read(cx).status.accepted.as_ref().unwrap().cells[0].revision,
                7
            );
            w.click(trigger.clone(), cx);
            w.click("status-edit-labels", cx);
            finish_open(&table, w, cx);
            assert_eq!(w.find("label-name-empty").value(), Some("Unassigned"));
            assert!(table.read(cx).busy());
            w.click("status-back-to-labels", cx);
            assert!(w.try_find("status-label-editor").is_none());
            assert!(w.find("status-edit-labels").visible());
            assert_eq!(w.find("status-empty").focused(), Some(true));
            w.press("escape", cx);
            assert_eq!(w.find(trigger).focused(), Some(true));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn pending_and_failed_color_keep_editor_selection_and_cell_revisions(cx: &mut TestAppContext) {
        let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
        cx.update(|cx| cx.set_reduce_motion(true));
        cx.update_window(handle, |_, w, cx| {
            table.update(cx, |table, cx| table.status.accept(palette(), cx));
            w.render_frame(cx);
            w.click(format!("check-{}", Uuid::from_u128(61)), cx);
            w.click("add-input-g-b", cx);
            w.input("Retained entry", cx);
            w.click(format!("status-{}", Uuid::from_u128(59)), cx);
            w.click("status-edit-labels", cx);
            finish_open(&table, w, cx);
            w.click("label-color-working", cx);
            assert!(w.find("status-label-colors").visible());
            w.press("escape", cx);
            assert!(w.try_find("status-label-colors").is_none());
            assert!(w.find("status-label-editor").visible());
            w.click("label-color-working", cx);
            w.click("label-use-color-#ffcb00", cx);
        })
        .unwrap();
        cx.update_window(handle, |_, w, cx| {
            assert!(table.read(cx).status.labels.saving);
            assert_eq!(w.find("status-back-to-labels").disabled(), Some(true));
            w.press("escape", cx);
            assert!(w.find("status-label-editor").visible());
            table.update(cx, |table, cx| {
                table.status.labels.request = None;
                let generation = table.status.generation;
                table.finish_status_label_save(
                    generation,
                    "working".into(),
                    StatusLabelEdit {
                        label: None,
                        color: Some("#ffcb00".into()),
                    },
                    Err(EditError::Transport),
                    Some(Ok(StatusLabels {
                        actor_id: Uuid::from_u128(700),
                        column_id: Uuid::from_u128(500),
                        choices: palette().choices,
                        can_edit: true,
                    })),
                    w,
                    cx,
                );
            });
            w.render_frame(cx);
            assert!(w.find("status-label-retry").visible());
            assert!(table.read(cx).status.labels.blocked_save);
            assert_eq!(table.read(cx).status.labels.queued.len(), 1);
            assert_eq!(w.find("add-input-g-b").value(), Some("Retained entry"));
            assert_eq!(table.read(cx).selection, [Uuid::from_u128(61)]);
            assert!(
                table
                    .read(cx)
                    .status
                    .accepted
                    .as_ref()
                    .unwrap()
                    .cells
                    .iter()
                    .all(|cell| cell.revision == 7 && cell.value.is_none())
            );
            assert!(
                table.read(cx).status.labels.request.is_none(),
                "failed saves never replay themselves"
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn editable_member_can_choose_status_but_cannot_manage_labels(cx: &mut TestAppContext) {
        let (handle, table) = visit_titles(cx, true, ["Ink", "Sketch"]);
        cx.update_window(handle, |_, w, cx| {
            table.update(cx, |table, cx| {
                let groups = table.read.groups().to_vec();
                table.read = MainTableRead::from_response(groups, 7, 1, true, false).unwrap();
                table.status.accept(palette(), cx);
            });
            w.render_frame(cx);
            w.click(format!("status-{}", Uuid::from_u128(59)), cx);
            assert!(w.find("status-choice-working").visible());
            assert!(w.try_find("status-edit-labels").is_none());
        })
        .unwrap();
    }
}
