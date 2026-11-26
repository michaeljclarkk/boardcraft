use gpui_kit::*;

struct Workbench;

impl Render for Workbench {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_4()
            .child("Rustday development workspace")
    }
}

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| Workbench))
            .expect("open Rustday development window");
    });
}
