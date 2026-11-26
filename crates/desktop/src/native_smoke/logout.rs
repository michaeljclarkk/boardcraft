//! Graphical checkpoints; credentials arrive only on stdin at the password step.
use super::*;
use crate::connected::Connected;

#[derive(Clone, Copy)]
enum Step {
    Identifier,
    Password,
    Board,
    Failed,
    Edit,
    Submit,
    Saved,
    SignedOut,
    Expired,
}

pub(crate) fn install(view: Entity<Connected>, mode: &str, w: &Window) {
    let step = match mode {
        "--logout-member" => Step::Board,
        "--logout-viewer" => Step::Identifier,
        "--logout-reopen" => Step::Expired,
        _ => unreachable!(),
    };
    schedule(view, mode == "--logout-viewer", step, w);
}

fn id(prefix: &str, n: u128) -> SharedString {
    format!("{prefix}-{}", Uuid::from_u128(n)).into()
}

fn board(w: &Window, viewer: bool, second: &str) {
    assert!(message(
        w,
        if viewer {
            "Board items · Read only"
        } else {
            "Board items"
        }
    ));
    let [first, last] = [42, 17].map(|n| {
        assert!(w.find_all(id("editor", n)).is_empty());
        let hits = w.find_all(id("title", n));
        assert_eq!(hits.len(), 1);
        assert!(hits[0].visible());
        hits.into_iter().next().unwrap()
    });
    assert_eq!(first.label(), Some("Review"));
    assert_eq!(last.label(), Some(second));
    assert!(first.bounds().bottom() <= last.bounds().top());
    assert_eq!(w.find_all("logout")[0].label(), Some("Log out"));
    assert!(w.find_all("identifier").is_empty() && w.find_all("password").is_empty());
}

fn identifier(w: &Window) {
    let field = &w.find_all("identifier")[0];
    assert!(field.visible() && field.value() == Some(""));
    assert_eq!(w.find_all("continue")[0].label(), Some("Next"));
    assert!(w.find_all("password").is_empty() && w.find_all("logout").is_empty());
    assert!(!message(w, "Board items") && !message(w, "Board items · Read only"));
    for n in [42, 17] {
        assert!(w.find_all(id("title", n)).is_empty() && w.find_all(id("editor", n)).is_empty());
    }
}

fn schedule(view: Entity<Connected>, viewer: bool, step: Step, w: &Window) {
    use self::Step::*;
    w.on_next_frame(move |w, cx| {
        let loads = view.read(cx).loads;
        let saves = view.read(cx).submissions;
        let next = match step {
            Identifier | SignedOut | Expired if !w.find_all("identifier").is_empty() => {
                identifier(w);
                assert_eq!(
                    message(w, "Session expired. Sign in again."),
                    matches!(step, Expired)
                );
                assert!(!message(w, "Could not log out") && !message(w, "Saved"));
                assert_eq!(loads, usize::from(!matches!(step, Identifier)));
                checkpoint(match step {
                    Identifier => "identifier",
                    Expired => "expired",
                    _ => "signed-out",
                });
                if !viewer {
                    w.remove_window();
                    return;
                }
                click(&w.find_all("identifier")[0], w, cx);
                w.input("boardmember", cx);
                click(&w.find_all("continue")[0], w, cx);
                Password
            }
            Password if !w.find_all("password").is_empty() => {
                assert_eq!(w.find_all("continue")[0].label(), Some("Sign in"));
                click(&w.find_all("password")[0], w, cx);
                println!("check password");
                let mut secret = String::new();
                std::io::stdin().read_line(&mut secret).unwrap();
                let secret = secret.trim_end();
                assert!(secret.len() == 32 && secret.bytes().all(|b| b.is_ascii_hexdigit()));
                w.input(secret, cx);
                click(&w.find_all("continue")[0], w, cx);
                Board
            }
            Board if !w.find_all(id("title", 17)).is_empty() => {
                board(w, viewer, "Review");
                assert_eq!(saves, 0);
                if loads == 2 && viewer {
                    checkpoint("signed-in-again");
                    w.remove_window();
                    return;
                }
                assert_eq!(loads, 1);
                checkpoint("board");
                click(&w.find_all("logout")[0], w, cx);
                if viewer { SignedOut } else { Failed }
            }
            Failed if message(w, "Could not log out") => {
                board(w, false, "Review");
                assert_eq!((saves, loads), (0, 1));
                checkpoint("failed");
                w.double_click(id("title", 17), cx);
                Edit
            }
            Edit => {
                editor(&w.find_all(id("editor", 17))[0], "Review");
                replace(" Jade ", w, cx);
                Submit
            }
            Submit => {
                editor(&w.find_all(id("editor", 17))[0], " Jade ");
                w.press("enter", cx);
                Saved
            }
            Saved if message(w, "Saved") => {
                board(w, false, "Jade");
                assert_eq!((saves, loads), (1, 1));
                checkpoint("saved");
                click(&w.find_all("logout")[0], w, cx);
                SignedOut
            }
            _ => step,
        };
        schedule(view, viewer, next, w);
    });
}
