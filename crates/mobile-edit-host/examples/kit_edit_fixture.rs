#[path = "../tests/fixture/mod.rs"]
mod fixture;

use boardcraft_account::Role;
use boardcraft_mobile_edit_client::SavedItem;
use fixture::{FIRST, Fixture, SECOND};
use std::io::{self, BufRead, Read, Write};

fn main() -> io::Result<()> {
    let mut input = io::stdin().lock();
    let f = Fixture::with_titles(["Review sketch", "Ink"]);
    {
        let mut line = String::new();
        input.by_ref().take(1027).read_line(&mut line)?;
        let password = line
            .strip_suffix('\n')
            .map(|s| s.trim_end_matches('\r'))
            .filter(|s| {
                !s.is_empty()
                    && s.len() <= 1024
                    && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            })
            .ok_or_else(|| io::Error::other("expected fixture credential on stdin"))?;
        f.set_creator(f.seed_account("boardmember", password, Role::Member));
    }
    let mut output = io::stdout().lock();
    writeln!(
        output,
        "{}",
        serde_json::json!({"board": f.board, "port": f.host.port, "schema": f.schema.name})
    )?;
    output.flush()?;
    for command in input.lines() {
        let (title, revision) = match command?.as_str() {
            "original" => ("Ink", 1),
            state @ ("rejected" | "signed-in") => {
                let count: i64 = f
                    .connect()
                    .query_one("SELECT count(*) FROM account_sessions", &[])
                    .unwrap()
                    .get(0);
                assert_eq!(count, i64::from(state == "signed-in"));
                ("Ink", 1)
            }
            "saved" => ("Coral", 2),
            _ => return Err(io::Error::other("unknown fixture state")),
        };
        f.check_store(&[
            SavedItem {
                id: FIRST,
                title: "Review sketch".into(),
                revision: 1,
            },
            SavedItem {
                id: SECOND,
                title: title.into(),
                revision,
            },
        ]);
        writeln!(output, "ok")?;
        output.flush()?;
    }
    Ok(())
}
