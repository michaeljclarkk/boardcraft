use super::{Window, fixture::Fixture, http_window_command, selection::Selection};

pub(super) fn run(revoked: bool) {
    let f = Fixture::start();
    let s = Selection::new(&f);
    let port = s.host.port;
    let mut cmd = http_window_command();
    cmd.args([
        "browse",
        &port.to_string(),
        if revoked {
            "--browse-revoked"
        } else {
            "--browse-open"
        },
    ])
    .env(
        "BOARDCRAFT_SESSION_ORIGIN",
        format!("http://127.0.0.1:{port}"),
    )
    .env("BOARDCRAFT_SESSION_TOKEN", s.token.encoded());
    let mut w = Window::launch(cmd);
    w.expect("listed");
    s.check(&f, true);
    if revoked {
        s.membership(&f, false);
        w.reply("ok");
        w.expect("unavailable");
        s.check(&f, false);
        w.reply("ok");
        w.expect("empty");
        s.check(&f, false);
        s.membership(&f, true);
        w.reply("ok");
        w.expect("restored");
        s.check(&f, true);
    }
    w.reply("ok");
    w.expect("selected");
    s.check(&f, true);
    if !revoked {
        w.reply("ok");
        w.expect("listed-again");
        s.check(&f, true);
        w.reply("ok");
        w.expect("sibling");
        s.check(&f, true);
    }
    w.finish();
}
