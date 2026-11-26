//! Drive the mounted browse controls. The only source of a pasted address is Ctrl+V.
use super::{
    navigation::{controls, id},
    *,
};

enum Step {
    Listed,
    Selected,
    Share,
    Copy,
    Pasted,
    NewWindow,
    Recipient,
    WrongOrigin,
    Unsupported,
    Corrected,
    Denied,
    Recovered,
}

pub(crate) fn install(mode: &str, port: u16, w: &Window) {
    drive(
        if mode == "--link-recipient" {
            Step::Recipient
        } else {
            Step::Listed
        },
        port,
        mode == "--link-window",
        w,
    );
}

fn button(key: &'static str, w: &mut Window, cx: &mut App) {
    click(&w.find_all(key)[0], w, cx);
}

fn selected(w: &Window) {
    assert!(message(w, "Board items · Read only"));
    controls(w, false, &[(73, "Sketch"), (61, "Ink")]);
}

fn drive(step: Step, port: u16, open_window: bool, w: &Window) {
    use Step::*;
    w.on_next_frame(move |w, cx| {
        // Oracle only; never type this value into the destination field.
        let expected = format!("http://127.0.0.1:{port}/boards/{}", Uuid::from_u128(201));
        let next = match step {
            Listed if message(w, "Boards") => {
                controls(w, true, &[]);
                click(&w.find_all(id("board", 201))[0], w, cx);
                Selected
            }
            Selected if message(w, "Board items · Read only") => {
                selected(w);
                checkpoint("selected");
                button("share", w, cx);
                Share
            }
            Share if message(w, "Recipients need existing access to this board.") => {
                assert!(message(w, "Board"));
                assert!(w.find_all("share-address")[0].value() == Some(&expected));
                button("copy-link", w, cx);
                Copy
            }
            Copy if message(w, "Link copied") => {
                button("board-link", w, cx);
                w.press("ctrl-v", cx);
                Pasted
            }
            Pasted => {
                assert!(w.find_all("board-link")[0].value() == Some(&expected));
                checkpoint("copied");
                if !open_window {
                    w.remove_window();
                    return;
                }
                button("new-window", w, cx);
                NewWindow
            }
            NewWindow
                if let Some(other) = cx.windows().into_iter().find(|h| *h != w.window_handle()) =>
            {
                let source = w.window_handle();
                cx.update_window(other, |_, w, _| destination(source, w))
                    .unwrap();
                return;
            }
            Recipient if message(w, "No boards") => {
                controls(w, false, &[]);
                button("board-link", w, cx);
                w.press("ctrl-v", cx);
                // Start the two invalid submissions from the real pasted input.
                WrongOrigin
            }
            WrongOrigin => {
                let field = &w.find_all("board-link")[0];
                let pasted = field.value().unwrap();
                assert!(pasted == expected);
                let other: u16 = std::env::var("BOARDCRAFT_FIXTURE_OTHER_PORT")
                    .unwrap()
                    .parse()
                    .unwrap();
                replace(
                    &pasted.replacen(&format!(":{port}/"), &format!(":{other}/"), 1),
                    w,
                    cx,
                );
                button("open-link", w, cx);
                Unsupported
            }
            Unsupported if message(w, "Enter a board link from this connection.") => {
                checkpoint("wrong-origin");
                button("board-link", w, cx);
                replace("unsupported address", w, cx);
                button("open-link", w, cx);
                Corrected
            }
            Corrected if message(w, "Enter a board link from this connection.") => {
                assert_eq!(
                    w.find_all("board-link")[0].value(),
                    Some("unsupported address")
                );
                button("board-link", w, cx);
                w.press("ctrl-a", cx);
                w.press("ctrl-v", cx);
                button("open-link", w, cx);
                Denied
            }
            Denied if message(w, "Destination unavailable") => {
                controls(w, false, &[]);
                for key in ["share", "copy-link", "new-window", "share-address"] {
                    assert!(w.find_all(key).is_empty());
                }
                assert!(w.find_all("board-link")[0].value() == Some(&expected));
                checkpoint("denied");
                button("open-link", w, cx);
                Recovered
            }
            Recovered if message(w, "Board items · Read only") => {
                selected(w);
                checkpoint("reopened");
                w.remove_window();
                return;
            }
            _ => step,
        };
        drive(next, port, open_window, w);
    });
}

fn destination(source: AnyWindowHandle, w: &Window) {
    w.on_next_frame(move |w, cx| {
        if !message(w, "Board items · Read only") {
            destination(source, w);
            return;
        }
        assert!(source != w.window_handle());
        selected(w);
        cx.update_window(source, |_, w, _| {
            selected(w);
            assert!(message(w, "Recipients need existing access to this board."));
            checkpoint("opened");
            w.remove_window();
        })
        .unwrap();
        w.remove_window();
    });
}
