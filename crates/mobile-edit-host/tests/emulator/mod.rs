use boardcraft_account::Token;
use boardcraft_mobile_edit_client::{EditError, SavedItem};
use std::{
    env,
    io::{self, Read, Seek, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
use tempfile::tempfile;
use uuid::Uuid;
use wait_timeout::ChildExt;

pub struct Adb {
    serial: String,
    server: u16,
    transport: u16,
}

impl Adb {
    pub fn dedicated() -> Self {
        let serial = env::var("RUSTDAY_ANDROID_SERIAL").expect("explicit emulator serial required");
        assert!(
            serial
                .strip_prefix("emulator-")
                .is_some_and(|s| { !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) }),
            "expected emulator serial"
        );
        let transport = serial[9..]
            .parse::<u16>()
            .ok()
            .filter(|&console| console >= 5554 && console % 2 == 0)
            .expect("expected even emulator console port")
            + 1;
        let server = env::var("ADB_SERVER_SOCKET")
            .ok()
            .and_then(|s| s.strip_prefix("tcp:")?.parse::<u16>().ok())
            .filter(|p| *p != 0 && *p != 5037)
            .expect("set ADB_SERVER_SOCKET=tcp:<dedicated loopback server port>");
        let adb = Self {
            serial,
            server,
            transport,
        };
        adb.preflight().unwrap();
        adb
    }

    fn preflight(&self) -> io::Result<()> {
        let name = self.run(&["emu", "avd", "name"])?;
        if name.lines().next().map(str::trim) != Some("Rustday_Edit_X64") {
            return Err(io::Error::other("expected Rustday_Edit_X64"));
        }
        for (prop, expected) in [
            ("ro.boot.qemu.avd_name", "Rustday_Edit_X64"),
            ("sys.boot_completed", "1"),
            ("ro.product.cpu.abi", "x86_64"),
        ] {
            if self.run(&["shell", "getprop", prop])?.trim() != expected {
                return Err(io::Error::other(format!("expected {prop}={expected}")));
            }
        }
        Ok(())
    }

    fn run(&self, args: &[&str]) -> io::Result<String> {
        self.run_with_input(args, None)
    }

    fn run_with_input(&self, args: &[&str], token: Option<&Token>) -> io::Result<String> {
        // File-backed streams cannot deadlock waiting for pipe space or an
        // inherited pipe handle. Wait, then read only the small fixture reply.
        let mut stdout = tempfile()?;
        let mut child = Command::new("adb")
            .arg("-P")
            .arg(self.server.to_string())
            .arg("-s")
            .arg(&self.serial)
            .args(args)
            .stdin(if token.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(stdout.try_clone()?)
            .stderr(Stdio::inherit())
            .spawn()?;
        if let Some(token) = token
            && let Err(error) = child
                .stdin
                .take()
                .unwrap()
                .write_all(token.encoded().as_bytes())
        {
            let _ = child.kill();
            child.wait()?;
            return Err(error);
        }
        match child.wait_timeout(Duration::from_secs(20)) {
            Ok(Some(status)) if status.success() => (),
            Ok(Some(status)) => return Err(io::Error::other(format!("adb {}: {status}", args[0]))),
            result => {
                let _ = child.kill();
                child.wait()?;
                return Err(result
                    .err()
                    .unwrap_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "adb timed out")));
            }
        }
        stdout.rewind()?;
        if stdout.metadata()?.len() > 65536 {
            return Err(io::Error::other("oversized adb reply"));
        }
        let mut text = String::new();
        stdout.read_to_string(&mut text)?;
        Ok(text)
    }

    fn mappings_to(&self, host: &str) -> io::Result<Vec<String>> {
        let list = self.run(&["reverse", "--list"])?;
        Ok(list
            .lines()
            .filter_map(|line| {
                let parts = line.split_whitespace().collect::<Vec<_>>();
                match parts.as_slice() {
                    [_, device, target] if *target == host => Some((*device).to_owned()),
                    _ => None,
                }
            })
            .collect())
    }

    // adb tracks --no-rebind tcp:0 under the literal key "tcp:0" on the
    // host, even after the actual allocated device listener has been removed.
    // Retire that transport (not the server) before releasing the fixture.
    fn release_transport(&self) -> io::Result<()> {
        if !self.run(&["reverse", "--list"])?.trim().is_empty()
            || !self.run(&["forward", "--list"])?.trim().is_empty()
        {
            return Err(io::Error::other(
                "cannot reconnect a transport with other mappings",
            ));
        }
        self.run(&["reconnect"])?;
        self.run(&["wait-for-disconnect"])?;
        // Repeat the selected emulator's startup notification on the private
        // server. USB, mDNS and automatic emulator discovery remain disabled.
        let socket = SocketAddr::from((Ipv4Addr::LOCALHOST, self.server));
        let timeout = Duration::from_secs(5);
        let mut stream = TcpStream::connect_timeout(&socket, timeout)?;
        stream.set_write_timeout(Some(timeout))?;
        let request = format!("host:emulator:{}", self.transport);
        write!(stream, "{:04x}{request}", request.len())?;
        drop(stream); // host:emulator has no reply; verify the new transport instead.
        self.run(&["wait-for-device"])?;
        self.preflight()
    }
}

pub struct Probe {
    token: Token,
    adb: Adb,
    path: String,
    host: String,
    device_port: Option<u16>,
    attempted_reverse: bool,
}

impl Probe {
    pub fn install(adb: Adb, host_port: u16, token: Token) -> Self {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let target = env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("target"));
        let binary = workspace
            .join(target)
            .join("x86_64-linux-android/debug/android-edit-probe");
        assert!(
            binary.is_file(),
            "build android-edit-probe for x86_64-linux-android first"
        );
        let mut p = Self {
            token,
            adb,
            path: format!(
                "/data/local/tmp/boardcraft-edit-{}",
                Uuid::new_v4().simple()
            ),
            host: format!("tcp:{host_port}"),
            device_port: None,
            attempted_reverse: false,
        };
        p.adb
            .run(&["push", binary.to_str().unwrap(), &p.path])
            .unwrap();
        p.adb.run(&["shell", "chmod", "700", &p.path]).unwrap();
        assert!(
            p.adb.mappings_to(&p.host).unwrap().is_empty(),
            "host port already mapped"
        );
        p.attempted_reverse = true;
        let port = p
            .adb
            .run(&["reverse", "--no-rebind", "tcp:0", &p.host])
            .unwrap();
        let port: u16 = port.trim().parse().expect("expected allocated device port");
        assert_ne!(port, 0);
        p.device_port = Some(port);
        p
    }

    fn invoke(&self, board: Uuid, extra: &[&str]) -> String {
        let port = self.device_port.unwrap().to_string();
        let board = board.to_string();
        let mut args = vec!["shell", "-T", &self.path, &port, &board];
        args.extend_from_slice(extra);
        self.adb.run_with_input(&args, Some(&self.token)).unwrap()
    }

    pub fn read(&self, board: Uuid) -> Vec<SavedItem> {
        serde_json::from_str(&self.invoke(board, &[])).expect("expected Android board read")
    }

    pub fn rename(
        &self,
        board: Uuid,
        row: &SavedItem,
        title: &str,
    ) -> Result<SavedItem, EditError> {
        // adb shell joins its arguments for the remote shell. Preserve the
        // entire draft, including outer whitespace, as one literal argument.
        let draft = format!("'{}'", title.replace('\'', "'\\''"));
        let args = [&*row.id.to_string(), &*row.revision.to_string(), &draft];
        serde_json::from_str(&self.invoke(board, &args)).expect("expected Android rename result")
    }

    fn remove_reverse(&self) -> io::Result<()> {
        if !self.attempted_reverse {
            return Ok(());
        }
        // A lost allocation reply can still leave a mapping. There were no
        // mappings to this fixture port before the request; never remove-all.
        let mappings = self.adb.mappings_to(&self.host)?;
        match mappings.as_slice() {
            [] if self.device_port.is_none() => Ok(()),
            [device]
                if device
                    .strip_prefix("tcp:")
                    .and_then(|s| s.parse::<u16>().ok())
                    .is_some_and(|port| {
                        port != 0 && self.device_port.is_none_or(|p| p == port)
                    }) =>
            {
                self.adb.run(&["reverse", "--remove", device])?;
                Ok(())
            }
            _ => Err(io::Error::other("cannot identify owned reverse mapping")),
        }
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        let mut identity = self.adb.preflight();
        // A failed probe may coincide with transport loss. Only cleanup can
        // wait for reattachment; never replay a probe or its assertions.
        if identity.is_err() && std::thread::panicking() {
            let _ = self.adb.run(&["wait-for-device"]);
            identity = self.adb.preflight();
        }
        let result = identity.and_then(|()| {
            let reverse = self.remove_reverse();
            let file = self.adb.run(&["shell", "rm", "-f", &self.path]);
            reverse.and(file)?;
            if self.attempted_reverse {
                self.adb.release_transport()?;
            }
            Ok(())
        });
        if let Err(err) = result {
            if std::thread::panicking() {
                eprintln!(
                    "also failed cleanup of {} and reverse to {}: {err}",
                    self.path, self.host
                );
            } else {
                panic!("device cleanup failed: {err}");
            }
        }
    }
}
