use super::{
    SmokeProcess, connected_command,
    fixture::{FIRST, Fixture, SECOND},
    lines,
    peer::Peer,
    stored_session::assert_no_session,
    wait_for_close,
};
use boardcraft_account::Role;
use boardcraft_mobile_edit_client::{Credentials, EditClient, EditError, NameEdit, SavedItem};
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::{atomic::Ordering::SeqCst, mpsc::Receiver},
    time::Duration,
};
use uuid::Uuid;
mod relay;

struct Window(SmokeProcess, Receiver<String>);

impl Window {
    fn launch(mut cmd: Command) -> Self {
        let mut child = SmokeProcess(cmd.stdin(Stdio::piped()).spawn().unwrap());
        let rx = lines(&mut child);
        Self(child, rx)
    }
    fn expect(&self, name: &str) {
        let line = self
            .1
            .recv_timeout(Duration::from_secs(45))
            .unwrap_or_else(|error| panic!("waiting for {name}: {error}"));
        assert!(line == format!("check {name}"), "expected {name}");
    }
    fn reply(&mut self, reply: &str) {
        writeln!(self.0.0.stdin.as_mut().unwrap(), "{reply}").unwrap();
    }
    fn finish(mut self) {
        self.reply("ok");
        wait_for_close(&mut self.0);
    }
}

fn rows(saved: bool) -> [SavedItem; 2] {
    [
        (FIRST, "Review", 1),
        if saved {
            (SECOND, "Jade", 2)
        } else {
            (SECOND, "Review", 1)
        },
    ]
    .map(|(id, title, revision)| SavedItem {
        id,
        title: title.into(),
        revision,
    })
}

fn credentials(password: &str) -> Credentials {
    Credentials {
        identifier: "boardmember".into(),
        password: password.into(),
    }
}

pub(super) fn configured() {
    let f = Fixture::start();
    let password = Uuid::new_v4().simple().to_string();
    let member = f.seed_account("boardmember", &password, Role::Member);
    f.set_creator(member);
    let token = boardcraft_account::sign_in(&f.schema.config, "boardmember", &password).unwrap();
    let a = EditClient::with_bearer(f.host.port, token.encoded()).unwrap();
    let b = Peer::sign_in(&f, member, credentials(&password));
    let relay = relay::Relay::start(f.host.port);
    let launch = |mode| {
        let mut cmd = connected_command(relay.port, f.board, mode);
        cmd.env(
            "BOARDCRAFT_SESSION_ORIGIN",
            format!("http://127.0.0.1:{}", relay.port),
        )
        .env("BOARDCRAFT_SESSION_TOKEN", token.encoded());
        Window::launch(cmd)
    };
    let mut w = launch("--logout-member");
    let check = |saved, revoked: bool| {
        let expected = rows(saved);
        b.check(&f, !revoked);
        b.client.resolve().unwrap();
        assert_eq!(b.client.read(f.board).unwrap(), expected);
        if revoked {
            assert_eq!(a.resolve(), Err(EditError::SessionRejected));
            assert_eq!(a.read(f.board), Err(EditError::SessionRejected));
            assert_eq!(
                a.rename(
                    f.board,
                    SECOND,
                    &NameEdit {
                        title: "Amber".into(),
                        revision: 2
                    }
                ),
                Err(EditError::SessionRejected)
            );
        } else {
            assert_eq!(a.read(f.board).unwrap(), expected);
        }
        f.check_store(&expected);
    };
    w.expect("board");
    check(false, false);
    relay.cut.store(true, SeqCst);
    w.reply("ok");
    w.expect("failed");
    assert_eq!(relay.dropped.load(SeqCst), 1);
    check(false, false);
    relay.cut.store(false, SeqCst);
    w.reply("ok");
    w.expect("saved");
    check(true, false);
    w.reply("ok");
    w.expect("signed-out");
    check(true, true);
    w.finish();
    let cold = launch("--logout-reopen");
    cold.expect("expired");
    check(true, true);
    cold.finish();
}

pub(super) fn interactive() {
    let f = Fixture::start();
    let password = Uuid::new_v4().simple().to_string();
    f.set_creator(f.seed_account("owner", &password, Role::Member));
    let viewer = f.seed_account("boardmember", &password, Role::Viewer);
    f.connect()
        .execute(
            "INSERT INTO private_board_subscribers(board_id,account_id) VALUES ($1,$2)",
            &[&f.board, &viewer],
        )
        .unwrap();
    let mut w = Window::launch(connected_command(f.host.port, f.board, "--logout-viewer"));
    for (step, reply) in [("identifier", "ok"), ("password", password.as_str())] {
        w.expect(step);
        f.check_store(&rows(false));
        assert_no_session(&f);
        w.reply(reply);
    }
    w.expect("board");
    f.check_store(&rows(false));
    let peer = Peer::sign_in(&f, viewer, credentials(&password));
    peer.check(&f, true);
    w.reply("ok");
    for (step, reply) in [("signed-out", "ok"), ("password", password.as_str())] {
        w.expect(step);
        f.check_store(&rows(false));
        peer.check(&f, false);
        peer.client.resolve().unwrap();
        assert_eq!(peer.client.read(f.board).unwrap(), rows(false));
        w.reply(reply);
    }
    w.expect("signed-in-again");
    peer.check_sessions(&f, 2, false); // peer + a new Viewer verifier, never the old one
    assert_eq!(peer.client.read(f.board).unwrap(), rows(false));
    f.check_store(&rows(false));
    w.finish();
}
