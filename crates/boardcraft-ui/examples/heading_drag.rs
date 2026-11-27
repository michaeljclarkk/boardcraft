//! Run with `cargo run -p boardcraft-ui --example heading_drag`.
//! Name Buttons retain keyboard focus; Escape cancels a held drag. Applications
//! should also offer their keyboard order command in a stock Kit menu.
use boardcraft_ui::HeadingDrag;
use gpui_kit::{
    base::ElementExt,
    component::{button::Button, label::Label},
    *,
};
use std::{cell::RefCell, rc::Rc};

struct Sample {
    ids: Vec<SharedString>,
    drag: Rc<RefCell<HeadingDrag>>,
}
impl Render for Sample {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let geometry = self.drag.clone();
        let mut content = div()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .on_prepaint(move |b, _, _| geometry.borrow_mut().set_viewport(b))
            .capture_key_down(cx.listener(|s, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    s.drag.borrow_mut().cancel();
                    cx.notify();
                }
            }));
        for id in &self.ids {
            let geometry = self.drag.clone();
            let key = id.clone();
            content = content.child(
                div()
                    .on_prepaint(move |b, _, _| {
                        let mut d = geometry.borrow_mut();
                        let s = d.section_mut(&key).unwrap();
                        s.set_name(b);
                        s.set_heading(b);
                        s.set_section(b);
                    })
                    .child(Button::new(id.clone()).label(id.clone())),
            );
        }
        if let Some(id) = self.drag.borrow().moving_id() {
            content = content.child(Label::new(format!("Moving {id}")));
        }
        let view = cx.weak_entity();
        content.child(
            canvas(
                |_, _, _| (),
                move |_, _, w, _| {
                    let down = view.clone();
                    w.on_mouse_event(move |e: &MouseDownEvent, phase, w, cx| {
                        if phase == DispatchPhase::Capture
                            && e.button == MouseButton::Left
                            && !w.last_input_was_touch()
                        {
                            _ = down.update(cx, |s, _| s.drag.borrow_mut().press(e.position));
                        }
                    });
                    let moving = view.clone();
                    w.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                        if phase == DispatchPhase::Capture {
                            _ = moving.update(cx, |s, cx| {
                                s.drag.borrow_mut().move_pointer(e.position);
                                cx.notify();
                            });
                        }
                    });
                    let up = view.clone();
                    w.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Capture && e.button == MouseButton::Left {
                            _ = up.update(cx, |s, cx| {
                                let ids = s.drag.borrow_mut().release(e.position);
                                if s.drag.borrow_mut().consume_click() {
                                    cx.stop_propagation();
                                }
                                if let Some(ids) = ids {
                                    s.drag = Rc::new(RefCell::new(HeadingDrag::new(
                                        ids.iter().cloned(),
                                    )));
                                    s.ids = ids;
                                }
                                cx.notify();
                            });
                        }
                    });
                },
            )
            .absolute()
            .size_full(),
        )
    }
}
fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
                cx.new(|_| {
                    let ids = vec!["Alpha".into(), "Beta".into(), "Empty".into()];
                    Sample {
                        drag: Rc::new(RefCell::new(HeadingDrag::new(ids.iter().cloned()))),
                        ids,
                    }
                })
            })
            .expect("open synthetic drag sample");
        });
}
