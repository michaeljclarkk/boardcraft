use boardcraft_board::{Board, BoardId, Item, RenameError, store};
use boardcraft_desktop::{Failure, InlineEdit, Phase, database};
use gpui_kit::component::{
    button::Button,
    input::{Input, InputEvent, InputState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use postgres::{Config, NoTls};
use uuid::Uuid;

#[cfg(all(windows, feature = "native-smoke"))]
mod native_smoke;

struct Row {
    cfg: Config,
    edit: InlineEdit,
    input: Entity<InputState>,
    display_focus: FocusHandle,
    // Present only while the Input is mounted. Take it before moving focus so
    // cancellation and Enter cannot be submitted again by the ensuing blur.
    baseline: Option<String>,
    // Once a different stored title is observed, only an explicit completion
    // may retry against it. Moving focus must not overwrite the other writer.
    conflicted: bool,
    #[cfg(all(windows, feature = "native-smoke"))]
    submissions: usize,
    _events: Subscription,
}

impl Row {
    fn new(
        cfg: Config,
        board: BoardId,
        item: Item,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let edit = InlineEdit::new(board, item);
        let input =
            cx.new(|cx| InputState::new(window, cx).default_value(edit.accepted().to_owned()));
        let events = cx.subscribe_in(
            &input,
            window,
            |this, input, event, window, cx| match event {
                InputEvent::Change if this.baseline.is_some() => {
                    this.edit.edit(input.read(cx).value().to_string());
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => this.complete(true, window, cx),
                InputEvent::Blur
                    if !this.conflicted && !matches!(this.edit.phase(), Phase::Failed(_)) =>
                {
                    this.complete(false, window, cx)
                }
                _ => {}
            },
        );
        Self {
            cfg,
            edit,
            input,
            display_focus: cx.focus_handle(),
            baseline: None,
            conflicted: false,
            #[cfg(all(windows, feature = "native-smoke"))]
            submissions: 0,
            _events: events,
        }
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.baseline.is_some() || self.edit.phase() == &Phase::Pending {
            return;
        }
        self.baseline = Some(self.edit.accepted().to_owned());
        self.input.update(cx, |input, cx| {
            input.set_value(self.edit.draft().to_owned(), window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.baseline = None;
        self.conflicted = false;
        self.edit.revert();
        self.display_focus.focus(window, cx);
        cx.notify();
    }

    fn complete(&mut self, return_focus: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(baseline) = self.baseline.take() else {
            return;
        };
        let raw = self.input.read(cx).value().to_string();
        self.conflicted = false;
        if return_focus {
            self.display_focus.focus(window, cx);
        }
        if raw == baseline {
            self.edit.revert();
            cx.notify();
            return;
        }
        self.edit.edit(raw);
        let Some(save) = self.edit.submit() else {
            cx.notify();
            return;
        };
        let cfg = self.cfg.clone();
        #[cfg(all(windows, feature = "native-smoke"))]
        {
            self.submissions += 1;
        }
        let work = cx
            .background_executor()
            .spawn(async move { save.connect_and_run(&cfg) });
        cx.spawn_in(window, async move |row, cx| {
            let result = work.await;
            _ = row.update_in(cx, |row, window, cx| {
                row.edit.finish(result);
                if matches!(row.edit.phase(), Phase::Failed(_)) {
                    row.conflicted =
                        matches!(row.edit.phase(), Phase::Failed(Failure::Conflict(_)));
                    row.open(window, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for Row {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let phase = self.edit.phase();
        let message = match phase {
            Phase::Clean => "",
            Phase::Editing => "Unsaved draft",
            Phase::Pending => "Saving…",
            Phase::Saved => "Saved",
            Phase::Failed(Failure::Rejected(RenameError::BlankTitle)) => {
                "Not saved: enter a title."
            }
            Phase::Failed(Failure::Rejected(RenameError::TitleTooLong)) => {
                "Not saved: limit is 255 UTF-16 units."
            }
            Phase::Failed(Failure::Rejected(RenameError::MissingItem(_))) => {
                "Not saved: item no longer exists on this board."
            }
            Phase::Failed(Failure::Database) => "Save failed. Check the connection and retry.",
            Phase::Failed(Failure::Conflict(current)) => {
                return self.controls(
                    format!("Changed elsewhere: {current}. Revert to use it, or retry your draft."),
                    cx,
                );
            }
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
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .when_else(
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
                            line.on_key_down(cx.listener(
                                |this, event: &KeyDownEvent, window, cx| {
                                    if event.keystroke.key == "f2"
                                        && this.display_focus.is_focused(window)
                                    {
                                        this.open(window, cx);
                                        cx.stop_propagation();
                                    }
                                },
                            ))
                            .child(
                                Button::new(self.title_id())
                                    .label(self.edit.accepted().to_owned())
                                    .track_focus(&self.display_focus)
                                    .on_click(cx.listener(
                                        |this, event: &ClickEvent, window, cx| {
                                            this.display_focus.focus(window, cx);
                                            if event.click_count() == 2 {
                                                this.open(window, cx);
                                            }
                                        },
                                    )),
                            )
                        },
                    )
                    .when(
                        self.conflicted || matches!(self.edit.phase(), Phase::Failed(_)),
                        |line| {
                            line.child(
                                Button::new("retry").label("Retry").on_click(
                                    cx.listener(|this, _, w, cx| this.complete(true, w, cx)),
                                ),
                            )
                            .child(
                                Button::new("revert")
                                    .label("Revert")
                                    .on_click(cx.listener(|this, _, w, cx| this.discard(w, cx))),
                            )
                        },
                    ),
            )
            .child(component::label::Label::new(message))
    }
}

struct Workbench {
    board: BoardId,
    rows: Vec<Entity<Row>>,
    message: &'static str,
    failed: bool,
}

impl Workbench {
    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.message = "Loading…";
        self.failed = false;
        let id = self.board;
        let work = cx.background_executor().spawn(async move {
            let cfg = database()?;
            let board = cfg
                .connect(NoTls)
                .and_then(|mut db| store::load_board(&mut db, id))
                .map_err(|e| {
                    eprintln!("load_board board={} sqlstate={:?}", id.as_uuid(), e.code());
                    "Cannot load board. Check the database connection and schema."
                })?
                .ok_or("Board not found.")?;
            Ok::<(Config, Board), &str>((cfg, board))
        });
        cx.spawn_in(window, async move |view, cx| {
            let loaded = work.await;
            _ = view.update_in(cx, |view, window, cx| {
                match loaded {
                    Ok((cfg, board)) => {
                        view.message = if board.items().is_empty() {
                            "Empty board"
                        } else {
                            "Double-click a name or focus it and press F2. Enter to finish; Escape to discard."
                        };
                        view.rows = board
                            .items()
                            .iter()
                            .map(|item| {
                                cx.new(|cx| Row::new(cfg.clone(), id, item.clone(), window, cx))
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
            .child(self.message)
            .when(self.failed, |el| {
                el.child(
                    Button::new("reload")
                        .label("Retry loading")
                        .on_click(cx.listener(|view, _, window, cx| view.load(window, cx))),
                )
            })
            .children(self.rows.iter().cloned())
    }
}

fn main() {
    let Some(board) = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<Uuid>().ok())
        .and_then(BoardId::new)
    else {
        eprintln!(
            "Usage: boardcraft-desktop <board-uuid> (run the seed_board example for a new synthetic board)"
        );
        std::process::exit(2);
    };
    application().with_assets(assets::Assets).run(move |cx| {
        init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let (_window, _board_view) = open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                let mut view = Workbench {
                    board,
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
