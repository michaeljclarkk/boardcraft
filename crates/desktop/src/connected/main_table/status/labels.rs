use super::*;
use boardcraft_mobile_edit_client::{StatusLabelEdit, StatusLabels};
use boardcraft_ui::{LabelEntry, STATUS_LABEL_COLORS, StatusLabelEditor, StatusLabelEvent};
use std::collections::VecDeque;

pub(super) struct Labels {
    owner: WeakEntity<Connected>,
    visit: Uuid,
    board: Uuid,
    column: Uuid,
    client: EditClient,
    port: u16,
    editor: Entity<StatusLabelEditor>,
    queue: VecDeque<(String, StatusLabelEdit)>,
    pending: bool,
    failed: bool,
    pub(super) active: bool,
    _events: Subscription,
    _draft_events: Subscription,
    draft_path: Option<std::path::PathBuf>,
    retained: Option<Vec<u8>>,
    storage_blocked: bool,
    close_requested: bool,
    popup: WeakEntity<PopoverState>,
    item: Uuid,
}

fn entry(c: &StatusChoice) -> LabelEntry {
    LabelEntry {
        key: c.key.clone(),
        label: c.label.clone(),
        color: c.color.clone(),
    }
}

impl Labels {
    pub(super) fn owns_other_cell(&self, item: Uuid) -> bool {
        self.active && self.item != item
    }
    fn new(
        owner: WeakEntity<Connected>,
        visit: Uuid,
        board: Uuid,
        column: Uuid,
        client: EditClient,
        port: u16,
        popup: WeakEntity<PopoverState>,
        item: Uuid,
        choices: Vec<StatusChoice>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor =
            cx.new(|cx| StatusLabelEditor::new(choices.iter().map(entry).collect(), w, cx));
        let events = cx.subscribe_in(&editor, w, |this, _, event, w, cx| match event {
            StatusLabelEvent::Edit { key, label, color } => {
                if !this.persist(cx) {
                    return;
                }
                let patch = StatusLabelEdit {
                    label: label.clone(),
                    color: color.clone(),
                };
                if let Some((_, queued)) = this.queue.iter_mut().find(|(id, _)| id == key) {
                    if patch.label.is_some() {
                        queued.label = patch.label;
                    }
                    if patch.color.is_some() {
                        queued.color = patch.color;
                    }
                } else {
                    this.queue.push_back((key.clone(), patch));
                }
                if !this.failed {
                    this.save_next(w, cx);
                }
            }
            StatusLabelEvent::New => {
                let count = this.editor.read(cx).entries().len();
                let label = LabelEntry {
                    key: format!("l-{}", Uuid::new_v4()),
                    label: "New label".into(),
                    color: STATUS_LABEL_COLORS[count % STATUS_LABEL_COLORS.len()].into(),
                };
                this.editor.update(cx, |e, cx| e.insert(label, w, cx));
            }
            StatusLabelEvent::Retry => this.check_access(true, w, cx),
            StatusLabelEvent::Recover => this.check_access(true, w, cx),
            StatusLabelEvent::Discard => this.discard_draft(w, cx),
        });
        let draft_events = cx.observe(&editor, |this, _, cx| {
            this.persist(cx);
        });
        Self {
            owner,
            visit,
            board,
            column,
            client,
            port,
            editor,
            queue: VecDeque::new(),
            pending: false,
            failed: false,
            active: true,
            _events: events,
            _draft_events: draft_events,
            draft_path: None,
            retained: None,
            storage_blocked: false,
            close_requested: false,
            popup,
            item,
        }
    }

    pub(super) fn blocked(&self, cx: &App) -> bool {
        self.pending || self.editor.read(cx).has_drafts()
    }
    fn update_choices(&self, choices: Vec<StatusChoice>, cx: &mut App) {
        _ = self.owner.update(cx, |owner, cx| {
            if let Screen::Navigation(nav) = &mut owner.screen
                && let Page::Items(_, t) = &mut nav.page
                && t.status.visit == self.visit
                && let ColumnRead::Ready {
                    choices: accepted, ..
                } = &mut t.status.read
            {
                *accepted = choices.into();
                cx.notify();
            }
        });
    }
    fn check_access(&mut self, _retry: bool, w: &mut Window, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        self.pending = true;
        self.editor.update(cx, |e, cx| e.set_pending(true, cx));
        let client = self.client.clone();
        let board = self.board;
        let column = self.column;
        let work = cx
            .background_executor()
            .spawn(async move { client.read_status_labels(board, column) });
        cx.spawn_in(w, async move |this, cx| {
            let result = work.await;
            _ = this.update_in(cx, |this, w, cx| {
                this.pending = false;
                this.editor.update(cx, |e, cx| e.set_pending(false, cx));
                match result {
                    Ok(labels) if labels.can_edit => {
                        if !this.restore(labels.actor_id, w, cx) {
                            cx.notify();
                            return;
                        }
                        this.failed = false;
                        for choice in &labels.choices {
                            this.editor
                                .update(cx, |e, cx| e.accept_in(entry(choice), w, cx));
                        }
                        this.update_choices(labels.choices, cx);
                        this.editor.update(cx, |e, cx| e.set_error(None, cx));
                        // Restored source drafts resume after an authorized read;
                        // Retry performs this same deliberate read-and-resend once.
                        // Build from the latest retained fields, including edits
                        // made while an earlier patch was in flight.
                        this.queue = this
                            .editor
                            .read(cx)
                            .drafts()
                            .into_iter()
                            .map(|(key, label, color)| (key, StatusLabelEdit { label, color }))
                            .collect();
                        this.save_next(w, cx);
                    }
                    Ok(_) => this.fail("Only an Owner can edit Status labels.", cx),
                    Err(_) => this.fail("Could not load labels. Your changes are kept. Retry.", cx),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn fail(&mut self, message: &str, cx: &mut Context<Self>) {
        self.failed = true;
        self.editor
            .update(cx, |e, cx| e.set_error(Some(message.into()), cx));
    }
    fn save_next(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.pending || self.failed || !self.persist(cx) {
            return;
        }
        let Some((key, patch)) = self.queue.pop_front() else {
            self.settle_close(w, cx);
            return;
        };
        self.pending = true;
        self.editor.update(cx, |e, cx| e.set_pending(true, cx));
        let client = self.client.clone();
        let board = self.board;
        let column = self.column;
        let sent_key = key.clone();
        let sent_patch = StatusLabelEdit {
            label: patch.label.clone(),
            color: patch.color.clone(),
        };
        let work = cx.background_executor().spawn(async move {
            match client.save_status_label(board, column, &sent_key, &sent_patch) {
                Ok(saved) => Ok(saved),
                Err(error) => Err((error, client.read_status_labels(board, column))),
            }
        });
        cx.spawn_in(w, async move |this, cx| {
            let result = work.await;
            _ = this.update_in(cx, |this, w, cx| {
                this.pending = false;
                this.editor.update(cx, |e, cx| e.set_pending(false, cx));
                match result {
                    Ok(saved) => {
                        this.editor.update(cx, |e, cx| {
                            e.accept_in(entry(&saved), w, cx);
                            e.set_error(None, cx);
                        });
                        this.publish_saved(saved, cx);
                        this.save_next(w, cx);
                    }
                    Err((_, Ok(labels))) if labels.can_edit => {
                        let confirmed = labels
                            .choices
                            .iter()
                            .find(|c| c.key == key)
                            .filter(|c| {
                                patch.label.as_ref().is_none_or(|v| v == &c.label)
                                    && patch.color.as_ref().is_none_or(|v| v == &c.color)
                            })
                            .cloned();
                        this.update_choices(labels.choices, cx);
                        if let Some(saved) = confirmed {
                            this.editor
                                .update(cx, |e, cx| e.accept_in(entry(&saved), w, cx));
                            this.save_next(w, cx);
                        } else {
                            this.queue.push_front((key.clone(), patch.clone()));
                            this.fail("Label not saved. Your changes are kept. Retry.", cx);
                        }
                    }
                    Err((EditError::PermissionDenied, _))
                    | Err((
                        _,
                        Ok(StatusLabels {
                            can_edit: false, ..
                        }),
                    )) => {
                        this.queue.push_front((key.clone(), patch.clone()));
                        this.fail(
                            "Only an Owner can edit Status labels. Your changes are kept.",
                            cx,
                        );
                    }
                    Err(_) => {
                        this.queue.push_front((key.clone(), patch.clone()));
                        this.fail(
                            "Label save was not confirmed. Your changes are kept. Retry.",
                            cx,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn publish_saved(&self, saved: StatusChoice, cx: &mut App) {
        _ = self.owner.update(cx, |owner, cx| {
            if let Screen::Navigation(nav) = &mut owner.screen
                && let Page::Items(_, t) = &mut nav.page
                && t.status.visit == self.visit
                && let ColumnRead::Ready { choices, .. } = &mut t.status.read
            {
                let mut next = choices.to_vec();
                if let Some(choice) = next.iter_mut().find(|c| c.key == saved.key) {
                    *choice = saved;
                } else {
                    next.push(saved);
                }
                *choices = next.into();
                cx.notify();
            }
        });
    }

    fn restore(&mut self, actor: Uuid, w: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.draft_path.is_some() && !self.storage_blocked {
            return true;
        }
        let Some(root) = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from) else {
            self.fail(
                "Could not retain label drafts locally. Keep this editor open and Retry.",
                cx,
            );
            return false;
        };
        let path = root.join("boardcraft/status-label-drafts").join(format!(
            "{}-{actor}-{}-{}.json",
            self.port, self.board, self.column
        ));
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => {
                self.storage_blocked = true;
                self.fail(
                    "Saved label drafts could not be read. Original bytes are kept. Retry.",
                    cx,
                );
                return false;
            }
        };
        self.draft_path = Some(path);
        self.retained = bytes.clone();
        let current = self.editor.read(cx).drafts();
        if let Some(bytes) = bytes {
            if bytes.len() > 262_144 {
                self.storage_blocked = true;
                self.editor
                    .update(cx, |e, cx| e.set_recovery_blocked(true, cx));
                self.fail("The saved label draft is still unreadable.", cx);
                return false;
            }
            let parsed =
                serde_json::from_slice::<Vec<(String, Option<String>, Option<String>)>>(&bytes);
            let Ok(patches) = parsed else {
                self.storage_blocked = true;
                self.editor
                    .update(cx, |e, cx| e.set_recovery_blocked(true, cx));
                self.fail("The saved label draft is still unreadable.", cx);
                return false;
            };
            let mut keys = std::collections::HashSet::new();
            if patches.len() > 250
                || patches.iter().any(|(key, name, color)| {
                    key.is_empty()
                        || key.len() > 80
                        || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                        || !keys.insert(key.clone())
                        || (name.is_none() && color.is_none())
                        || name
                            .as_ref()
                            .is_some_and(|n| n.encode_utf16().count() > 160)
                        || color.as_ref().is_some_and(|c| {
                            c.len() != 7
                                || !c.starts_with('#')
                                || !c[1..].bytes().all(|b| b.is_ascii_hexdigit())
                        })
                })
            {
                self.storage_blocked = true;
                self.editor
                    .update(cx, |e, cx| e.set_recovery_blocked(true, cx));
                self.fail("The saved label draft is still unreadable.", cx);
                return false;
            }
            self.editor.update(cx, |e, cx| {
                for (key, label, color) in patches {
                    e.restore_patch(key, label, color, w, cx);
                }
            });
        }
        self.editor.update(cx, |e, cx| {
            for (key, label, color) in current {
                e.restore_patch(key, label, color, w, cx);
            }
        });
        self.storage_blocked = false;
        self.editor
            .update(cx, |e, cx| e.set_recovery_blocked(false, cx));
        true
    }

    fn persist(&mut self, cx: &mut Context<Self>) -> bool {
        if self.storage_blocked {
            return false;
        }
        let Some(path) = &self.draft_path else {
            return true;
        };
        let patches = self.editor.read(cx).drafts();
        let next = if patches.is_empty() {
            None
        } else {
            serde_json::to_vec(&patches).ok()
        };
        if next == self.retained {
            return true;
        }
        let current = match std::fs::read(path) {
            Ok(v) => Some(v),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => {
                self.storage_blocked = true;
                self.fail(
                    "Could not retain label drafts locally. Keep this editor open and Retry.",
                    cx,
                );
                return false;
            }
        };
        if current != self.retained {
            self.storage_blocked = true;
            self.fail(
                "Saved label drafts changed in another window. Reopen the board before retrying.",
                cx,
            );
            return false;
        }
        let result = (|| -> std::io::Result<()> {
            if let Some(bytes) = &next {
                let parent = path.parent().unwrap();
                std::fs::create_dir_all(parent)?;
                let mut temp = tempfile::NamedTempFile::new_in(parent)?;
                std::io::Write::write_all(&mut temp, bytes)?;
                temp.persist(path).map_err(|e| e.error)?;
            } else if current.is_some() {
                std::fs::remove_file(path)?;
            }
            Ok(())
        })();
        if result.is_err() {
            self.storage_blocked = true;
            self.fail(
                "Could not retain label drafts locally. Keep this editor open and Retry.",
                cx,
            );
            return false;
        }
        self.retained = next;
        true
    }

    fn discard_draft(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = &self.draft_path else {
            return;
        };
        let current = match std::fs::read(path) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => {
                self.fail("The saved label draft could not be discarded. Retry.", cx);
                return;
            }
        };
        if current != self.retained {
            self.fail(
                "The saved label draft changed. Retry recovery before discarding it.",
                cx,
            );
            return;
        }
        if current.is_some() && std::fs::remove_file(path).is_err() {
            self.fail("The saved label draft could not be discarded. Retry.", cx);
            return;
        }
        self.retained = None;
        self.storage_blocked = false;
        self.failed = false;
        self.editor.update(cx, |e, cx| {
            e.set_recovery_blocked(false, cx);
            e.set_error(None, cx);
        });
        self.settle_close(w, cx);
        cx.notify();
    }

    pub(super) fn request_close(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.editor.update(cx, |e, cx| e.close_palette(w, cx)) {
            return;
        }
        self.close_requested = true;
        self.settle_close(w, cx);
    }
    fn settle_close(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.close_requested && !self.blocked(cx) {
            self.active = false;
            self.close_requested = false;
            _ = self.popup.update(cx, |p, cx| p.dismiss(w, cx));
            _ = self.owner.update(cx, |_, cx| cx.notify());
            cx.notify();
        }
    }
}

impl Render for Labels {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let blocked = self.blocked(cx);
        div()
            .flex()
            .flex_col()
            .gap_3()
            .w_full()
            .child(self.editor.clone())
            .child(
                div()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .pt_2()
                    .child(
                        Button::new("status-back-to-labels")
                            .ghost()
                            .w_full()
                            .label("Back to labels")
                            .disabled(blocked)
                            .on_click(cx.listener(|this, _, w, cx| {
                                this.active = false;
                                cx.notify();
                                _ = this.popup.update(cx, |_, cx| cx.notify());
                                _ = this.owner.update(cx, |owner, cx| {
                                    if let Screen::Navigation(nav) = &owner.screen
                                        && let Page::Items(_, t) = &nav.page
                                        && let ColumnRead::Ready { cells, .. } = &t.status.read
                                        && let Some(cell) = cells.get(&this.item)
                                    {
                                        cell.selected.focus(w, cx);
                                    }
                                    cx.notify();
                                });
                            })),
                    ),
            )
    }
}

impl StatusColumn {
    pub(in crate::connected::main_table) fn label_edits_active(&self, cx: &App) -> bool {
        self.labels
            .as_ref()
            .is_some_and(|e| e.read(cx).active || e.read(cx).blocked(cx))
    }
}

impl Connected {
    pub(super) fn open_status_labels(
        &mut self,
        target: Target,
        popup: WeakEntity<PopoverState>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.weak_entity();
        let notify_popup = popup.clone();
        let port = self.port;
        let Screen::Navigation(nav) = &mut self.screen else {
            return;
        };
        let Page::Items(board, t) = &mut nav.page else {
            return;
        };
        if t.status.visit != target.visit || !t.manageable {
            return;
        }
        if let Some(labels) = &t.status.labels {
            labels.update(cx, |labels, cx| {
                labels.active = true;
                labels.popup = popup;
                labels.item = target.item;
                labels.close_requested = false;
                labels.check_access(false, w, cx);
                cx.notify();
            });
        } else if let ColumnRead::Ready { choices, .. } = &t.status.read {
            let labels = cx.new(|cx| {
                Labels::new(
                    owner,
                    target.visit,
                    *board,
                    target.column,
                    nav.client.clone(),
                    port,
                    popup,
                    target.item,
                    choices.to_vec(),
                    w,
                    cx,
                )
            });
            labels.update(cx, |labels, cx| labels.check_access(false, w, cx));
            t.status.labels = Some(labels);
        }
        cx.notify();
        _ = notify_popup.update(cx, |_, cx| cx.notify());
    }
}
