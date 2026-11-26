//! Observe only the controls needed for the next gesture. Never dump a window or input value.
use super::*;

#[derive(Clone, Copy)]
enum Stage {
    Identifier,
    Password,
    Rejected,
    Board,
}

pub(crate) fn install(board: Uuid, w: &Window) {
    schedule_login(Stage::Identifier, board, w);
}

fn schedule_login(stage: Stage, board: Uuid, w: &Window) {
    w.on_next_frame(move |w, cx| {
        let rows = [42, 17]
            .map(|n| w.find_all(SharedString::from(format!("title-{}", Uuid::from_u128(n)))));
        if !matches!(stage, Stage::Board) {
            assert!(rows.iter().all(Vec::is_empty));
        }
        let action = w.find_all("continue");
        let next = match stage {
            Stage::Identifier if !action.is_empty() => {
                assert_eq!(action[0].label(), Some("Next"));
                assert!(w.find_all("password").is_empty());
                click(&w.find_all("identifier")[0], w, cx);
                w.input("boardmember", cx);
                click(&action[0], w, cx);
                Stage::Password
            }
            Stage::Password | Stage::Rejected
                if !action.is_empty()
                    && (matches!(stage, Stage::Password)
                        || message(w, "Incorrect username or password")) =>
            {
                assert!(w.find_all("identifier").is_empty());
                assert_eq!(action[0].label(), Some("Sign in"));
                click(&w.find_all("password")[0], w, cx);
                if matches!(stage, Stage::Password) {
                    // Deliberately different length from the generated fixture password.
                    w.input(&Uuid::new_v4().to_string(), cx);
                } else {
                    println!("check rejected");
                    let mut secret = String::new();
                    std::io::stdin().read_line(&mut secret).unwrap();
                    let secret = secret.trim_end();
                    assert!(secret.len() == 32 && secret.bytes().all(|b| b.is_ascii_hexdigit()));
                    w.input(secret, cx);
                }
                click(&action[0], w, cx);
                if matches!(stage, Stage::Password) {
                    Stage::Rejected
                } else {
                    Stage::Board
                }
            }
            Stage::Board if rows.iter().any(|hits| !hits.is_empty()) => {
                assert!(
                    action.is_empty()
                        && w.find_all("identifier").is_empty()
                        && w.find_all("password").is_empty()
                );
                assert!(message(w, "Board items · Read only"));
                assert!(rows.iter().all(|hits| hits.len() == 1 && hits[0].visible()));
                assert!(rows[0][0].bounds().bottom() <= rows[1][0].bounds().top());
                for (hits, title) in rows.iter().zip(["Review sketch", "Ink"]) {
                    assert_eq!(hits[0].label(), Some(title));
                }
                println!("opened {board}");
                w.remove_window();
                return;
            }
            _ => stage,
        };
        schedule_login(next, board, w);
    });
}
