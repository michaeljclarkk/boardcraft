//! Observe the mounted controls, then dispatch a single pointer click at their bounds.
use super::*;

#[derive(Clone, Copy)]
enum Step {
    Listed,
    Unavailable,
    Empty,
    Restored,
    Selected,
    Again,
    Sibling,
}

pub(crate) fn install(revoked: bool, w: &Window) {
    schedule(Step::Listed, revoked, w);
}

pub(super) fn id(prefix: &str, n: u128) -> SharedString {
    format!("{prefix}-{}", Uuid::from_u128(n)).into()
}

// Check every possible catalog and item control in this fixture, including absence.
pub(super) fn controls(w: &Window, listed: bool, items: &[(u128, &str)]) {
    let expected_boards = if listed {
        &[(203, "Quartz"), (202, "Review"), (201, "Review")][..]
    } else {
        &[]
    };
    for (prefix, all, expected) in [
        ("board", &[201, 202, 203, 204][..], expected_boards),
        ("title", &[73, 61, 62, 42, 17][..], items),
    ] {
        for &n in all {
            assert_eq!(
                w.find_all(id(prefix, n)).len(),
                usize::from(expected.iter().any(|&(k, _)| k == n))
            );
        }
        let mut bottom = None;
        for &(n, label) in expected {
            let hits = w.find_all(id(prefix, n));
            let hit = &hits[0];
            assert!(hit.visible());
            assert_eq!(hit.label(), Some(label));
            assert!(bottom.is_none_or(|b| b <= hit.bounds().top()));
            bottom = Some(hit.bounds().bottom());
        }
    }
    for n in [73, 61, 62, 42, 17] {
        assert!(w.find_all(id("editor", n)).is_empty());
    }
    for key in ["save", "logout", "identifier", "password", "continue"] {
        assert!(w.find_all(key).is_empty());
    }
}

fn schedule(step: Step, revoked: bool, w: &Window) {
    use self::Step::*;
    w.on_next_frame(move |w, cx| {
        let next = match step {
            Listed | Again | Restored if message(w, "Boards") => {
                controls(w, true, &[]);
                assert_eq!(w.find_all("board-list")[0].label(), Some("Refresh"));
                checkpoint(match step {
                    Listed => "listed",
                    Again => "listed-again",
                    _ => "restored",
                });
                click(
                    &w.find_all(id("board", if matches!(step, Again) { 202 } else { 201 }))[0],
                    w,
                    cx,
                );
                match step {
                    Again => Sibling,
                    Listed if revoked => Unavailable,
                    _ => Selected,
                }
            }
            Unavailable if message(w, "Destination unavailable") => {
                controls(w, false, &[]);
                assert_eq!(w.find_all("board-list")[0].label(), Some("Back to boards"));
                checkpoint("unavailable");
                click(&w.find_all("board-list")[0], w, cx);
                Empty
            }
            Empty if message(w, "No boards") => {
                controls(w, false, &[]);
                assert_eq!(w.find_all("board-list")[0].label(), Some("Refresh"));
                checkpoint("empty");
                click(&w.find_all("board-list")[0], w, cx);
                Restored
            }
            Selected | Sibling if message(w, "Board items") => {
                let sibling = matches!(step, Sibling);
                controls(
                    w,
                    false,
                    if sibling {
                        &[(62, "Outline")]
                    } else {
                        &[(73, "Sketch"), (61, "Ink")]
                    },
                );
                assert_eq!(w.find_all("board-list")[0].label(), Some("Back to boards"));
                checkpoint(if sibling { "sibling" } else { "selected" });
                if sibling || revoked {
                    w.remove_window();
                    return;
                }
                click(&w.find_all("board-list")[0], w, cx);
                Again
            }
            _ => step,
        };
        schedule(next, revoked, w);
    });
}
