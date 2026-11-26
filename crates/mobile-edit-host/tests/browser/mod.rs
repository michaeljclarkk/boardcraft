use crate::{
    fixture::Fixture,
    selection::{Catalog, QUARTZ, SELECTED, SIBLING},
};
use boardcraft_account::Role;
use boardcraft_board::BoardId;
use boardcraft_mobile_edit_host::Host;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::Duration,
};
use tempfile::TempDir;
use uuid::Uuid;
use wait_timeout::ChildExt;

pub struct Preview {
    driver: Driver,
    _host: Host,
    pub catalog: Catalog,
    pub f: Fixture,
    pub viewer: Uuid,
    pub password: String,
}

impl Preview {
    pub fn open(foreground: bool) -> Self {
        let f = Fixture::with_titles(["Review", "Review"]);
        let password = Uuid::new_v4().simple().to_string();
        let viewer = f.seed_account("reader", &password, Role::Viewer);
        let catalog = Catalog::new(&f, viewer);
        let host = Host::start_preview(&f.schema.config, BoardId::new(f.board).unwrap()).unwrap();
        let driver = Driver::start(host.port, foreground);
        let mut app = Self {
            driver,
            _host: host,
            catalog,
            f,
            viewer,
            password,
        };
        app.act("open");
        app
    }

    pub fn act(&mut self, action: &str) {
        let mut command = json!({"action": action});
        if matches!(
            action,
            "login" | "retry" | "failed logout" | "back" | "refresh restored"
        ) {
            // Read the catalog records and grant links directly, not the operation under test.
            self.catalog.check(&self.f, true);
            let rows: Vec<(Uuid, String, i32)> = self
                .f
                .connect()
                .query(
                    "SELECT p.board_id, p.name, p.position FROM board_policies p
                 JOIN board_subscribers s USING(board_id) WHERE s.account_id=$1 ORDER BY p.position",
                    &[&self.viewer],
                )
                .unwrap()
                .iter()
                .map(|r| (r.get(0), r.get(1), r.get(2)))
                .collect();
            assert_eq!(
                rows,
                [
                    (QUARTZ, "Quartz".into(), 0),
                    (SIBLING, "Review".into(), 1),
                    (SELECTED, "Review".into(), 2)
                ]
            );
            command["rows"] = json!(
                rows.into_iter()
                    .map(|(id, name, _)| (id, name))
                    .collect::<Vec<_>>()
            );
        }
        if action == "login" {
            command["password"] = json!(self.password);
        }
        self.driver.command(action, command);
    }
}

struct Driver {
    child: Child,
    replies: Receiver<bool>,
    profile: Option<TempDir>,
}

impl Driver {
    fn start(port: u16, foreground: bool) -> Self {
        let profile = tempfile::Builder::new()
            .prefix("boardcraft-chromium-")
            .tempdir()
            .unwrap();
        let mut command = Command::new("node");
        command.env_clear();
        for key in [
            "PATH",
            "SystemRoot",
            "SystemDrive",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "LOCALAPPDATA",
            "APPDATA",
            "USERPROFILE",
            "TMP",
            "TEMP",
            "HOME",
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "XDG_RUNTIME_DIR",
            "BOARDCRAFT_CHROMIUM",
        ] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        let mut child = command
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/browser/drive.mjs"
            ))
            .arg(profile.path())
            .arg(if foreground { "foreground" } else { "headless" })
            .arg(port.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start isolated browser driver");
        let output = child.stdout.take().unwrap();
        let (tx, replies) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if tx.send(line == "ok").is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            replies,
            profile: Some(profile),
        }
    }

    fn command(&mut self, action: &str, value: Value) {
        let stdin = self.child.stdin.as_mut().unwrap();
        serde_json::to_writer(&mut *stdin, &value).expect("send browser action");
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
        let ok = if action == "park" {
            self.replies.recv().ok()
        } else {
            self.replies.recv_timeout(Duration::from_secs(40)).ok()
        };
        assert!(ok == Some(true), "browser checkpoint: {action}");
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        let exited = self
            .child
            .wait_timeout(Duration::from_secs(15))
            .ok()
            .flatten();
        if exited.is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        let deleted = self.profile.take().unwrap().close().is_ok();
        if !deleted || !exited.is_some_and(|s| s.success()) {
            if std::thread::panicking() {
                eprintln!("Browser cleanup or driver exit failed");
            } else {
                panic!("Browser cleanup or driver exit failed");
            }
        }
    }
}
