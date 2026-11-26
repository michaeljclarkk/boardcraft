use super::{
    Window,
    fixture::Fixture,
    http_window_command,
    selection::{self, QUARTZ, SELECTED, SIBLING, Selection},
};
use boardcraft_account::{Account, Role, Token, resolve};
use std::{
    io::ErrorKind,
    net::{Ipv4Addr, TcpListener},
};
use uuid::Uuid;

fn launch(port: u16, mode: &str, token: &Token, other: Option<u16>) -> Window {
    let mut cmd = http_window_command();
    cmd.args(["browse", &port.to_string(), mode])
        .env(
            "BOARDCRAFT_SESSION_ORIGIN",
            format!("http://127.0.0.1:{port}"),
        )
        .env("BOARDCRAFT_SESSION_TOKEN", token.encoded());
    if let Some(port) = other {
        cmd.env("BOARDCRAFT_FIXTURE_OTHER_PORT", port.to_string());
    }
    Window::launch(cmd)
}

pub(super) fn open_window() {
    let f = Fixture::start();
    let s = Selection::new(&f);
    let mut w = launch(s.host.port, "--link-window", &s.token, None);
    for step in ["selected", "copied"] {
        w.expect(step);
        s.check(&f, true);
        w.reply("ok");
    }
    w.expect("opened");
    s.check(&f, true);
    w.finish();
}

pub(super) fn recipient() {
    let f = Fixture::start();
    let s = Selection::new(&f);
    let port = s.host.port;
    let reader = resolve(&f.schema.config, &s.token).unwrap();
    let pw = Uuid::new_v4().simple().to_string();
    let viewer = f.seed_account("recipient", &pw, Role::Viewer);
    let token = boardcraft_account::sign_in(&f.schema.config, "recipient", &pw).unwrap();
    let workspace: Uuid = f
        .connect()
        .query_one(
            "SELECT workspace_id FROM board_policies WHERE board_id=$1",
            &[&SELECTED],
        )
        .unwrap()
        .get(0);
    f.connect()
        .execute(
            "INSERT INTO board_subscribers VALUES ($1,$2)",
            &[&SELECTED, &viewer],
        )
        .unwrap();

    let check = |joined| {
        assert_eq!(resolve(&f.schema.config, &s.token).unwrap(), reader);
        assert_eq!(
            resolve(&f.schema.config, &token).unwrap(),
            Account {
                id: viewer,
                role: Role::Viewer
            }
        );
        let mut db = f.connect();
        assert_eq!(
            db.query_one("SELECT count(*) FROM account_sessions", &[])
                .unwrap()
                .get::<_, i64>(0),
            2
        );
        let mut pairs = |sql| {
            db.query(sql, &[])
                .unwrap()
                .into_iter()
                .map(|r| (r.get::<_, Uuid>(0), r.get::<_, Uuid>(1)))
                .collect::<Vec<_>>()
        };
        let mut subscriptions = [
            (SELECTED, reader.id),
            (SIBLING, reader.id),
            (QUARTZ, reader.id),
            (SELECTED, viewer),
        ];
        subscriptions.sort();
        assert_eq!(
            pairs("SELECT board_id,account_id FROM board_subscribers ORDER BY 1,2"),
            subscriptions
        );
        let mut memberships = vec![(workspace, reader.id)];
        if joined {
            memberships.push((workspace, viewer));
            memberships.sort();
        }
        assert_eq!(
            pairs("SELECT workspace_id,account_id FROM workspace_members ORDER BY 1,2"),
            memberships
        );
        f.check_board(SELECTED, &selection::selected_rows());
        f.check_board(
            SIBLING,
            &[boardcraft_mobile_edit_client::SavedItem {
                id: Uuid::from_u128(62),
                title: "Outline".into(),
                revision: 1,
            }],
        );
    };
    check(false);
    let mut sender = launch(port, "--link-sender", &s.token, None);
    sender.expect("selected");
    check(false);
    sender.reply("ok");
    sender.expect("copied");
    check(false);

    let other = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    other.set_nonblocking(true).unwrap();
    let mut recipient = launch(
        port,
        "--link-recipient",
        &token,
        Some(other.local_addr().unwrap().port()),
    );
    recipient.expect("wrong-origin");
    assert!(matches!(other.accept(), Err(e) if e.kind() == ErrorKind::WouldBlock));
    recipient.reply("ok");
    recipient.expect("denied");
    check(false);
    boardcraft_mobile_edit_client::EditClient::with_bearer(port, token.encoded())
        .unwrap()
        .resolve()
        .unwrap();
    assert_eq!(
        f.connect()
            .execute(
                "INSERT INTO workspace_members VALUES ($1,$2)",
                &[&workspace, &viewer]
            )
            .unwrap(),
        1
    );
    recipient.reply("ok");
    recipient.expect("reopened");
    check(true);
    recipient.finish();
    sender.finish();
}
