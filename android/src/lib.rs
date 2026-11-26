mod connection;

use boardcraft_mobile_edit_client::{EditClient, EditError, NameEdit, SavedItem};
use connection::{Connection, launch_connection};
use gpui_kit::component::{
    Disableable, Root, Sizable, Theme, ThemeMode,
    button::Button,
    input::{Input, InputEvent, InputState},
    label::Label,
};
use gpui_kit::*;
use gpui_mobile::android::{host, jni};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
struct Item {
    id: Uuid,
    title: String,
    // Absent on the offline, display-only fixture.
    revision: Option<i64>,
}

impl From<SavedItem> for Item {
    fn from(row: SavedItem) -> Self {
        Self {
            id: row.id,
            title: row.title,
            revision: Some(row.revision),
        }
    }
}

#[derive(Deserialize)]
struct Board {
    title: String,
    items: Vec<Item>,
}

struct BoardView {
    board: Board,
    connection: Option<Connection>,
    message: &'static str,
    editor: Option<Editor>,
}

struct Editor {
    id: Uuid,
    revision: Option<i64>,
    input: Entity<InputState>,
    pending: bool,
    error: Option<&'static str>,
    _changes: Subscription,
}

impl BoardView {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            board: Board {
                title: "Sample board".into(),
                items: Vec::new(),
            },
            connection: None,
            message: "",
            editor: None,
        };
        match launch_connection() {
            Ok(None) => {
                view.board = serde_json::from_str(include_str!(
                    "../../scripts/fixtures/android-kit-board.json"
                ))
                .expect("valid synthetic board fixture")
            }
            Err(_) => view.message = "Invalid board connection",
            Ok(Some(conn)) => {
                view.connection = Some(conn);
                view.message = "Loading…";
                let work = cx
                    .background_executor()
                    .spawn(async move { EditClient::new(conn.port).read(conn.board) });
                cx.spawn(async move |view, cx| {
                    let rows = work.await;
                    _ = view.update(cx, |view, cx| {
                        match rows {
                            Ok(rows) => {
                                view.board.items = rows.into_iter().map(Item::from).collect();
                                view.message = "";
                            }
                            Err(_) => view.message = "Could not load board. Reopen to retry.",
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
        }
        view
    }

    fn open_editor(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        if self.connection.is_some() && self.editor.is_some() {
            return;
        }
        if let Some(item) = self.board.items.iter().find(|item| item.id == id) {
            let input = cx.new(|cx| InputState::new(window, cx).default_value(item.title.clone()));
            let changes = cx.subscribe(&input, |view, _, event, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(e) = &mut view.editor
                    && e.error.take().is_some()
                {
                    cx.notify();
                }
            });
            self.editor = Some(Editor {
                id,
                revision: item.revision,
                input,
                pending: false,
                error: None,
                _changes: changes,
            });
            cx.notify();
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let (Some(conn), Some(editor)) = (self.connection, self.editor.as_mut()) else {
            return;
        };
        let Some(revision) = editor.revision else {
            return;
        };
        if editor.pending {
            return;
        }
        let id = editor.id;
        let request = NameEdit {
            revision,
            title: editor.input.read(cx).value().to_string(),
        };
        let blank = request.title.trim().is_empty();
        editor.pending = true;
        editor.error = None;
        let work = cx
            .background_executor()
            .spawn(async move { EditClient::new(conn.port).rename(conn.board, id, &request) });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            _ = view.update(cx, |view, cx| {
                let Some(editor) = view.editor.as_mut().filter(|e| e.id == id && e.pending) else {
                    return;
                };
                editor.pending = false;
                match result {
                    Ok(row)
                        if row.id == id
                            && let Some(item) =
                                view.board.items.iter_mut().find(|item| item.id == id) =>
                    {
                        *item = row.into();
                        view.editor = None;
                    }
                    result => {
                        editor.error = Some(match result {
                            Err(EditError::InvalidDraft) if blank => "Name required",
                            Err(EditError::InvalidDraft) => "Name too long",
                            Err(EditError::Conflict) => "Revision conflict. Reopen the board.",
                            Err(EditError::Missing) => "Item not found. Reopen the board.",
                            _ => "Save failed. Try again.",
                        })
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for BoardView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut column = div()
            .flex()
            .flex_col()
            .size_full()
            .p_8()
            .gap_6()
            .child(Label::new(self.board.title.clone()).text_xl());
        if !self.message.is_empty() {
            column = column.child(Label::new(self.message));
        }
        for item in &self.board.items {
            if let Some(editor) = self.editor.as_ref().filter(|e| e.id == item.id) {
                column = column
                    .child(Label::new("Editing name").text_lg())
                    .child(Input::new(&editor.input).large().disabled(editor.pending));
                if self.connection.is_some() {
                    column = column.child(
                        Button::new("save")
                            .large()
                            .disabled(editor.pending)
                            .label(if editor.pending { "Saving…" } else { "Save" })
                            .on_click(cx.listener(|view, _, _, cx| view.save(cx))),
                    );
                    if let Some(error) = editor.error {
                        column = column.child(Label::new(error));
                    }
                }
            } else {
                let id = item.id;
                column = column.child(
                    Button::new(SharedString::from(id.to_string()))
                        .large()
                        .label(item.title.clone())
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.open_editor(id, window, cx);
                        })),
                );
            }
        }
        column
    }
}

#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    host::set_gpu_backend(host::GpuBackend::Gl);
    jni::install_panic_hook();
    let _platform = jni::init_platform(&app);
    let platform = jni::shared_platform().expect("Android platform initialized");
    Application::with_platform(platform.into_rc()).run(|cx| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Light, None, cx);
        cx.open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(BoardView::new);
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("open board fixture window");
        cx.activate(true);
    });
}
