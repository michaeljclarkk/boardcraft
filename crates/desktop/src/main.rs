use boardcraft_board::{Board, BoardId, Item, RenameError, store};
use boardcraft_desktop::{Failure, InlineEdit, Phase, database};
use gpui_kit::component::{
    Disableable,
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
                InputEvent::Change => {
                    this.edit.edit(input.read(cx).value().to_string());
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => this.save(window, cx),
                _ => {}
            },
        );
        Self {
            cfg,
            edit,
            input,
            _events: events,
        }
    }

    fn sync_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| {
            input.set_value(self.edit.draft().to_owned(), window, cx)
        });
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit.edit(self.input.read(cx).value().to_string());
        let Some(save) = self.edit.submit() else {
            return;
        };
        let cfg = self.cfg.clone();
        let work = cx
            .background_executor()
            .spawn(async move { save.connect_and_run(&cfg) });
        cx.spawn_in(window, async move |row, cx| {
            let result = work.await;
            _ = row.update_in(cx, |row, window, cx| {
                row.edit.finish(result);
                if row.edit.phase() == &Phase::Saved {
                    row.sync_input(window, cx);
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
        let dirty = matches!(phase, Phase::Editing | Phase::Failed(_));
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
                    dirty,
                    format!("Changed elsewhere: {current}. Revert to use it, or retry your draft."),
                    cx,
                );
            }
        };
        self.controls(dirty, message.to_owned(), cx)
    }
}

impl Row {
    fn title_id(&self) -> SharedString {
        let (_, item) = self.edit.identity();
        format!("title-{}", item.as_uuid()).into()
    }

    fn controls(&self, dirty: bool, message: String, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div().flex_1().child(
                            Input::new(&self.input)
                                .id(self.title_id())
                                .disabled(self.edit.phase() == &Phase::Pending),
                        ),
                    )
                    .child(
                        Button::new("save")
                            .label(if matches!(self.edit.phase(), Phase::Failed(_)) {
                                "Retry"
                            } else {
                                "Save"
                            })
                            .disabled(!dirty)
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    )
                    .child(
                        Button::new("revert")
                            .label("Revert")
                            .disabled(!dirty)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit.revert();
                                this.sync_input(window, cx);
                                cx.notify();
                            })),
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
                            "Edit a title and press Enter or Save."
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
