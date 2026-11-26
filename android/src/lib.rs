use gpui_kit::component::{
    Root, Sizable, Theme, ThemeMode,
    button::Button,
    input::{Input, InputState},
    label::Label,
};
use gpui_kit::*;
use gpui_mobile::android::{host, jni};
use serde::Deserialize;

#[derive(Deserialize)]
struct Item {
    id: String,
    title: String,
}

#[derive(Deserialize)]
struct Board {
    title: String,
    items: Vec<Item>,
}

struct BoardView {
    board: Board,
    editor: Option<(String, Entity<InputState>)>,
}

impl BoardView {
    fn open_editor(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(item) = self.board.items.iter().find(|item| item.id == id) {
            let input = cx.new(|cx| InputState::new(window, cx).default_value(item.title.clone()));
            self.editor = Some((item.id.clone(), input));
            cx.notify();
        }
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
        for item in &self.board.items {
            if let Some((_, input)) = self.editor.as_ref().filter(|(id, _)| id == &item.id) {
                column = column
                    .child(Label::new("Editing name").text_lg())
                    .child(Input::new(input).large());
            } else {
                let id = item.id.clone();
                column = column.child(
                    Button::new(SharedString::from(id.clone()))
                        .large()
                        .label(item.title.clone())
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.open_editor(&id, window, cx);
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
            let board = serde_json::from_str(include_str!(
                "../../scripts/fixtures/android-kit-board.json"
            ))
            .expect("valid synthetic board fixture");
            let view = cx.new(|_| BoardView {
                board,
                editor: None,
            });
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("open board fixture window");
        cx.activate(true);
    });
}
