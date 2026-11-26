//! Opt-in foreground spot check. Close the isolated browser window to finish.
#[path = "../tests/browser/mod.rs"]
mod browser;
#[allow(dead_code)]
#[path = "../tests/fixture/mod.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../tests/fixture/selection.rs"]
mod selection;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/fixture/stored_session.rs"]
mod stored_session;

fn main() {
    let mut preview = browser::Preview::open(true);
    preview.act("login");
    println!("Signed in. Close the preview browser window to finish.");
    preview.act("park");
}
