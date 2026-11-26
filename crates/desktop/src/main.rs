use boardcraft_board::{
    BoardId, RenameError,
    store::{self, StoredItem},
};
use boardcraft_desktop::{Failure, InlineEdit, Phase, Save, database};
use gpui_kit::component::{
    button::Button,
    input::{Input, InputEvent, InputState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use postgres::{Config, NoTls};
use uuid::Uuid;
mod connected;
mod recovery;
use recovery::{Draft, Record, Recovery};

#[cfg(all(windows, feature = "native-smoke"))]
mod native_smoke;

struct Row {
    cfg: Config,
    edit: InlineEdit,
    input: Entity<InputState>,
    display_focus: FocusHandle,
    // Present only while the Input is mounted. Take it before moving focus so
    // cancellation and Enter cannot be submitted again by the ensuing blur.
    baseline: Option<(String, i64)>,
    draft: Option<Draft>,
    recovery: Option<Recovery>,
    draft_warning: Option<&'static str>,
    failure_token: Option<Uuid>,
    dismissing: bool,
    #[cfg(all(windows, feature = "native-smoke"))]
    submissions: usize,
    _events: Subscription,
}

impl EventEmitter<Save> for Row {}

#[derive(Default)]
struct RowRecovery {
    failure: Option<Record>,
    draft: Option<Draft>,
    draft_store: Option<Recovery>,
}

impl Row {
    fn new(
        cfg: Config,
        board: BoardId,
        item: StoredItem,
        local: RowRecovery,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let id = item.id;
        let mut edit = InlineEdit::new(board, item);
        let failure_token = local.failure.map(|r| {
            let (token, raw, reason) = r.into_failure(id);
            edit.restore_failure(raw, reason);
            token
        });
        let input =
            cx.new(|cx| InputState::new(window, cx).default_value(edit.accepted().to_owned()));
        let events = cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
            InputEvent::PressEnter { .. } => this.complete(true, window, cx),
            InputEvent::Blur => this.complete(false, window, cx),
            InputEvent::Change => this.remember(cx),
            _ => {}
        });
        Self {
            cfg,
            edit,
            input,
            display_focus: cx.focus_handle(),
            baseline: None,
            draft: local.draft,
            recovery: local.draft_store,
            draft_warning: None,
            failure_token,
            dismissing: false,
            #[cfg(all(windows, feature = "native-smoke"))]
            submissions: 0,
            _events: events,
        }
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dismissing || self.baseline.is_some() || self.edit.phase() == &Phase::Pending {
            return;
        }
        let (entry, revision, raw) = match &self.draft {
            Some(d) => (d.entry.clone(), d.revision, d.raw.clone()),
            None => (
                self.edit.accepted().to_owned(),
                self.edit.revision(),
                self.edit.accepted().to_owned(),
            ),
        };
        self.baseline = Some((entry, revision));
        self.input.update(cx, |input, cx| {
            input.set_value(raw, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn remember(&mut self, cx: &mut Context<Self>) {
        let Some((entry, revision)) = &self.baseline else {
            return;
        };
        let raw = self.input.read(cx).value().to_string();
        // set_value on entry/restore can also emit Change. An untouched input
        // is not a draft. Once changed, even returning to entry is recoverable.
        if raw
            == self
                .draft
                .as_ref()
                .map_or(entry.as_str(), |d| d.raw.as_str())
        {
            return;
        }
        self.replace_draft(Some(Draft {
            token: Uuid::new_v4(),
            raw,
            entry: entry.clone(),
            revision: *revision,
        }));
        cx.notify();
    }

    fn replace_draft(&mut self, next: Option<Draft>) {
        if self.draft.is_none() && next.is_none() {
            return;
        }
        if let Some(store) = &self.recovery
            && let Err(message) = store.replace(
                self.edit.identity().1,
                self.draft.as_ref().map(|d| d.token),
                next.clone(),
            )
        {
            self.draft_warning = Some(message);
            // Keep the live input usable. Do not attempt blind cleanup later.
            self.recovery = None;
        }
        self.draft = next;
    }

    fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.baseline = None;
        self.replace_draft(None);
        self.display_focus.focus(window, cx);
        cx.notify();
    }

    fn complete(&mut self, return_focus: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some((entry, revision)) = self.baseline.take() else {
            return;
        };
        let raw = self.input.read(cx).value().to_string();
        self.replace_draft(None);
        if return_focus {
            self.display_focus.focus(window, cx);
        }
        if let Some(save) = self.edit.submit(raw, &entry, revision) {
            cx.emit(save);
        }
        cx.notify();
    }
}

impl Render for Row {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let phase = self.edit.phase();
        let message = match phase {
            _ if self.baseline.is_some() => "Editing name",
            Phase::Clean | Phase::Failed(_) => "",
            Phase::Pending => "Saving…",
            Phase::Saved => "Saved",
        };
        self.controls(message.to_owned(), cx)
    }
}

impl Row {
    fn title_id(&self) -> SharedString {
        let (_, item) = self.edit.identity();
        format!("title-{}", item.as_uuid()).into()
    }

    fn input_id(&self) -> SharedString {
        format!("editor-{}", self.edit.identity().1.as_uuid()).into()
    }

    fn controls(&self, message: String, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().flex().gap_2().items_center().when_else(
                self.baseline.is_some(),
                |line| {
                    line.capture_action(cx.listener(
                        |this, _: &component::input::Escape, window, cx| {
                            this.discard(window, cx);
                            cx.stop_propagation();
                        },
                    ))
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.input).id(self.input_id())),
                    )
                },
                |line| {
                    line.on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "f2" && this.display_focus.is_focused(window) {
                            this.open(window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(
                        Button::new(self.title_id())
                            .label(self.edit.display().to_owned())
                            .track_focus(&self.display_focus)
                            .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                                this.display_focus.focus(window, cx);
                                if event.click_count() == 2 {
                                    this.open(window, cx);
                                }
                            })),
                    )
                },
            ))
            .child(component::label::Label::new(message))
            .when_some(self.draft_warning, |el, warning| el.child(warning))
    }
}

struct Workbench {
    board: BoardId,
    recovery: Option<Recovery>,
    rows: Vec<Entity<Row>>,
    message: &'static str,
    failed: bool,
}

impl Workbench {
    fn run_save(
        &mut self,
        row: Entity<Row>,
        save: Save,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cfg = row.read(cx).cfg.clone();
        let r = row.read(cx);
        let (_, item) = r.edit.identity();
        let raw = r.edit.request().expect("submitted name").to_owned();
        let token = r.failure_token;
        let recovery = self.recovery.clone();
        #[cfg(all(windows, feature = "native-smoke"))]
        row.update(cx, |r, _| r.submissions += 1);
        let work = cx.background_executor().spawn(async move {
            // A submitted change supersedes this occurrence before any database
            // read or write. A different window may already own a newer failure.
            let cleared = match (&recovery, token) {
                (Some(store), Some(token)) => store.replace::<Record>(item, Some(token), None),
                _ => Ok(None),
            };
            let result = save.connect_and_run(&cfg);
            let persisted = cleared.and_then(|_| match (&recovery, &result) {
                (Some(store), Err(reason)) => {
                    store.replace(item, None, Some(Record::from_error(raw, &reason.into())))
                }
                _ => Ok(None),
            });
            (result, persisted)
        });
        cx.spawn_in(window, async move |board, cx| {
            let (result, persisted) = work.await;
            _ = board.update(cx, |board, cx| {
                row.update(cx, |r, cx| {
                    r.failure_token = persisted.unwrap_or_else(|message| {
                        board.message = message;
                        None
                    });
                    r.edit.finish(result);
                    cx.notify();
                });
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn dismiss(&mut self, row: Entity<Row>, window: &mut Window, cx: &mut Context<Self>) {
        let r = row.read(cx);
        if r.dismissing || !matches!(r.edit.phase(), Phase::Failed(_)) {
            return;
        }
        let (_, item) = r.edit.identity();
        let Some((store, token)) = self.recovery.clone().zip(r.failure_token) else {
            row.update(cx, |r, cx| {
                r.edit.dismiss();
                cx.notify();
            });
            cx.notify();
            return;
        };
        // Don't race a new edit with deletion of its predecessor.
        if r.baseline.is_some() {
            return;
        }
        row.update(cx, |r, _| r.dismissing = true);
        let work = cx
            .background_executor()
            .spawn(async move { store.replace::<Record>(item, Some(token), None) });
        cx.spawn_in(window, async move |board, cx| {
            let result = work.await;
            _ = board.update(cx, |board, cx| {
                row.update(cx, |r, cx| {
                    r.dismissing = false;
                    match result {
                        Ok(_) => {
                            r.failure_token = None;
                            r.edit.dismiss();
                        }
                        Err(message) => board.message = message,
                    }
                    cx.notify();
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.message = "Loading…";
        self.failed = false;
        let id = self.board;
        let work = cx.background_executor().spawn(async move {
            let cfg = database()?;
            let board = cfg
                .connect(NoTls)
                .and_then(|mut db| store::load_items(&mut db, id))
                .map_err(|e| {
                    eprintln!("load_board board={} sqlstate={:?}", id.as_uuid(), e.code());
                    "Cannot load board. Check the database connection and schema."
                })?
                .ok_or("Board not found.")?;
            // Local recovery is optional. If unreadable, leave its bytes alone
            // for the lifetime of this view and still mount the accepted board.
            let local = Recovery::configured(id);
            let recovery = local.as_ref().ok().and_then(Option::as_ref);
            let failures = recovery.map(Recovery::load::<Record>).transpose();
            let drafts = recovery.map(Recovery::load::<Draft>).transpose();
            Ok::<_, &'static str>((cfg, board, local, failures, drafts))
        });
        cx.spawn_in(window, async move |view, cx| {
            let loaded = work.await;
            _ = view.update_in(cx, |view, window, cx| {
                match loaded {
                    Ok((cfg, board, local, failures, drafts)) => {
                        let warning = local.as_ref().err().or(failures.as_ref().err()).or(drafts.as_ref().err()).copied();
                        let recovery = local.unwrap_or_default();
                        view.recovery = recovery.clone().filter(|_| failures.is_ok());
                        let draft_store = recovery.filter(|_| drafts.is_ok());
                        let mut drafts = drafts.unwrap_or_default().unwrap_or_default();
                        let mut records = failures.unwrap_or_default().unwrap_or_default();
                        view.message = if let Some(message) = warning {
                            message
                        } else if board.is_empty() {
                            "Empty board"
                        } else if view.recovery.is_none() {
                            "Local actor not set. Drafts and failed changes last only in this window."
                        } else {
                            "Double-click a name or focus it and press F2. Enter to finish; Escape to discard."
                        };
                        let mut restored_focus = false;
                        view.rows = board.into_iter()
                            .map(|item| {
                                 let uuid = item.id.as_uuid();
                                 let draft = drafts.remove(&uuid);
                                 let restore = draft.is_some() && !restored_focus;
                                 let local = RowRecovery { failure: records.remove(&uuid), draft, draft_store: draft_store.clone() };
                                 let row = cx.new(|cx| Row::new(cfg.clone(), id, item, local, window, cx));
                                 if restore {
                                     row.update(cx, |r, cx| r.open(window, cx));
                                     restored_focus = true;
                                 }
                                 cx.subscribe_in(&row, window, |board, row, save: &Save, w, cx| {
                                     board.run_save(row.clone(), save.clone(), w, cx);
                                 }).detach();
                                 row
                            })
                            .collect();
                    }
                    Err(message) => {
                        view.message = message;
                        view.failed = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for Workbench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("board")
            .size_full()
            .overflow_y_scroll()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child("boardcraft · Board items")
            .child(component::label::Label::new(self.message))
            .when(self.failed, |el| {
                el.child(
                    Button::new("reload")
                        .label("Retry loading")
                        .on_click(cx.listener(|view, _, window, cx| view.load(window, cx))),
                )
            })
            .children(self.rows.iter().filter_map(|row| {
                let r = row.read(cx);
                let Phase::Failed(reason) = r.edit.phase() else {
                    return None;
                };
                let message = match reason {
                    Failure::Rejected(RenameError::BlankTitle) => {
                        "Not saved: enter a title.".to_owned()
                    }
                    Failure::Rejected(RenameError::TitleTooLong) => {
                        "Not saved: limit is 255 UTF-16 units.".into()
                    }
                    Failure::Rejected(RenameError::MissingItem(_)) => {
                        "Not saved: item no longer exists on this board.".into()
                    }
                    Failure::Database => "Save failed. Check the connection and retry.".into(),
                    Failure::Conflict(current) => {
                        format!("Changed elsewhere: {current}. Retry to replace this name.")
                    }
                };
                let id = r.edit.identity().1.as_uuid();
                let retry = row.clone();
                let dismiss = row.clone();
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .child(component::label::Label::new(format!(
                            "Name change: {}",
                            r.edit.accepted()
                        )))
                        .child(component::label::Label::new(message))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    Button::new(SharedString::from(format!("retry-{id}")))
                                        .label("Retry change")
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            retry.update(cx, |r, cx| {
                                                if !r.dismissing
                                                    && r.baseline.is_none()
                                                    && let Some(save) = r.edit.retry()
                                                {
                                                    cx.emit(save);
                                                    cx.notify();
                                                }
                                            });
                                        })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!("dismiss-{id}")))
                                        .label("Dismiss")
                                        .on_click(cx.listener(move |board, _, window, cx| {
                                            board.dismiss(dismiss.clone(), window, cx);
                                        })),
                                ),
                        ),
                )
            }))
            .children(self.rows.iter().cloned())
    }
}

fn main() {
    fn usage() -> ! {
        eprintln!("Usage: boardcraft-desktop <board-uuid> | connect <loopback-port> <board-uuid>");
        std::process::exit(2);
    }
    let mut args = std::env::args().skip(1);
    let first = args.next().unwrap_or_else(|| usage());
    let (port, board) = if first == "connect" {
        let port = args
            .next()
            .and_then(|s| s.parse::<u16>().ok())
            .filter(|n| *n != 0)
            .unwrap_or_else(|| usage());
        let board = args.next().unwrap_or_else(|| usage());
        if let Some(extra) = args.next()
            && !(cfg!(all(windows, feature = "native-smoke"))
                && extra == "--smoke-login"
                && args.next().is_none())
        {
            usage();
        }
        (Some(port), board)
    } else {
        (None, first)
    };
    let board = board
        .parse::<Uuid>()
        .ok()
        .and_then(BoardId::new)
        .unwrap_or_else(|| usage());
    application().with_assets(assets::Assets).run(move |cx| {
        init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        if let Some(port) = port {
            connected::open(port, board.as_uuid(), cx);
            return;
        }
        let (_window, _board_view) = open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                let mut view = Workbench {
                    board,
                    recovery: None,
                    rows: vec![],
                    message: "",
                    failed: false,
                };
                view.load(window, cx);
                view
            })
        })
        .expect("open boardcraft board window");
        cx.activate(true);
        #[cfg(all(windows, feature = "native-smoke"))]
        native_smoke::install(_window, _board_view, cx);
    });
}
