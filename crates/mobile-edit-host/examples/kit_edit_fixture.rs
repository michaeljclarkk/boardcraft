#[path = "../tests/fixture/mod.rs"]
mod fixture;

use boardcraft_mobile_edit_client::SavedItem;
use fixture::{FIRST, Fixture, SECOND};
use std::io::{self, BufRead, Write};

fn main() -> io::Result<()> {
    let f = Fixture::with_titles(["Review sketch", "Ink"]);
    let mut output = io::stdout().lock();
    writeln!(
        output,
        "{}",
        serde_json::json!({"board": f.board, "port": f.host.port})
    )?;
    output.flush()?;
    for command in io::stdin().lock().lines() {
        let (title, revision) = match command?.as_str() {
            "original" => ("Ink", 1),
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
