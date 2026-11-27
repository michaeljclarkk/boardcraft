use crate::process::text;
use anyhow::{Context, Result, ensure};
use std::{
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    process::Command,
    time::Duration,
};

const TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct Device {
    serial: String,
    server_port: u16,
}

impl Device {
    pub(crate) fn from_env() -> Result<Self> {
        Self::configured(|key| std::env::var(key).unwrap_or_default())
    }

    fn configured(get: impl Fn(&str) -> String) -> Result<Self> {
        let port = get("ANDROID_ADB_SERVER_PORT");
        let server_port: u16 = port
            .parse()
            .context("expected private loopback adb server")?;
        ensure!(
            server_port != 0
                && server_port != 5037
                && server_port.to_string() == port
                && get("ADB_SERVER_SOCKET") == format!("tcp:{port}")
                && get("ADB_USB") == "0"
                && get("ADB_EMU") == "0"
                && get("ADB_MDNS") == "0",
            "expected private loopback adb server with discovery disabled"
        );
        let serial = get("RUSTDAY_ANDROID_SERIAL");
        ensure!(
            serial.strip_prefix("emulator-").is_some_and(|port| {
                !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit())
            }),
            "set RUSTDAY_ANDROID_SERIAL to the dedicated emulator serial"
        );
        Ok(Self {
            serial,
            server_port,
        })
    }

    fn command(&self) -> Command {
        let mut command =
            Command::new(std::env::var_os("BOARDCRAFT_ADB").unwrap_or_else(|| "adb".into()));
        // An explicit numeric host prevents adb from starting a missing local daemon.
        command
            .args([
                "-H",
                "127.0.0.1",
                "-P",
                &self.server_port.to_string(),
                "-s",
                &self.serial,
            ])
            .env_remove("ADB_SERVER_SOCKET");
        command
    }

    fn probe(&self, args: &[&str], client_version: u32) -> Result<String> {
        // adb can replace a mismatched daemon even with -H. Reject before invoking it.
        ensure!(
            server_version(self.server_port)? == client_version,
            "adb client/server version mismatch; preserve the existing server"
        );
        text(self.command().args(args), TIMEOUT)
    }

    pub(crate) fn validate(&self) -> Result<()> {
        let version = text(self.command().arg("version"), TIMEOUT)?;
        let version = version
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("Android Debug Bridge version 1.0."))
            .context("unexpected adb client version")?
            .parse()?;
        validate_observations(
            &self.probe(&["emu", "avd", "name"], version)?,
            &self.probe(&["shell", "getprop", "sys.boot_completed"], version)?,
            &self.probe(&["shell", "getprop", "ro.product.cpu.abi"], version)?,
        )
    }
}

fn server_version(port: u16) -> Result<u32> {
    let mut socket =
        TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, port)), TIMEOUT)
            .context("private adb server must already be running")?;
    socket.set_read_timeout(Some(TIMEOUT))?;
    socket.set_write_timeout(Some(TIMEOUT))?;
    // The sole smart-socket request is read-only; no start/kill/transport discovery.
    socket.write_all(b"000Chost:version")?;
    let mut header = [0; 8];
    socket.read_exact(&mut header)?;
    ensure!(&header == b"OKAY0004", "unexpected adb version response");
    let mut version = [0; 4];
    socket.read_exact(&mut version)?;
    Ok(u32::from_str_radix(std::str::from_utf8(&version)?, 16)?)
}

fn validate_observations(name: &str, boot: &str, abi: &str) -> Result<()> {
    ensure!(
        name.lines().next().map(str::trim) == Some("Rustday_Edit_X64"),
        "expected Rustday_Edit_X64 AVD"
    );
    ensure!(boot.trim() == "1", "emulator has not booted");
    ensure!(abi.trim() == "x86_64", "expected x86_64 Android");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, net::TcpListener, thread};

    #[test]
    fn refuses_unscoped_shared_remote_and_discovering_adb_before_contact() {
        let base = BTreeMap::from([
            ("ANDROID_ADB_SERVER_PORT", "5039"),
            ("ADB_SERVER_SOCKET", "tcp:5039"),
            ("ADB_USB", "0"),
            ("ADB_EMU", "0"),
            ("ADB_MDNS", "0"),
            ("RUSTDAY_ANDROID_SERIAL", "emulator-5682"),
        ]);
        for (key, value) in [
            ("ADB_SERVER_SOCKET", "tcp:remote:5039"),
            ("ANDROID_ADB_SERVER_PORT", "5037"),
            ("ANDROID_ADB_SERVER_PORT", "05039"),
            ("ADB_USB", "1"),
            ("ADB_EMU", "1"),
            ("ADB_MDNS", "1"),
            ("RUSTDAY_ANDROID_SERIAL", ""),
            ("RUSTDAY_ANDROID_SERIAL", "emulator-"),
            ("RUSTDAY_ANDROID_SERIAL", "physical-123"),
        ] {
            let mut changed = base.clone();
            changed.insert(key, value);
            assert!(Device::configured(|k| changed.get(k).unwrap_or(&"").to_string()).is_err());
        }
        let device = Device::configured(|k| base.get(k).unwrap_or(&"").to_string()).unwrap();
        let command = device.command();
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            ["-H", "127.0.0.1", "-P", "5039", "-s", "emulator-5682"]
        );
    }

    #[test]
    fn mismatched_or_malformed_server_is_not_handed_to_adb() {
        for response in [b"OKAY00040001".as_slice(), b"FAIL0004nope"] {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
            let server_port = listener.local_addr().unwrap().port();
            let server = thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket.set_read_timeout(Some(TIMEOUT)).unwrap();
                let mut request = [0; 16];
                socket.read_exact(&mut request).unwrap();
                assert_eq!(&request, b"000Chost:version");
                socket.write_all(response).unwrap();
            });
            let device = Device {
                serial: "emulator-5682".into(),
                server_port,
            };
            assert!(
                device
                    .probe(&["shell", "getprop", "sys.boot_completed"], 41)
                    .is_err()
            );
            server.join().unwrap();
        }
    }

    #[test]
    fn refuses_wrong_avd_unbooted_and_wrong_device_abi() {
        assert!(validate_observations("Rustday_Edit_X64\r\r\nOK\r\r\n", "1\n", "x86_64\n").is_ok());
        for (name, boot, abi) in [
            ("another_avd\nOK", "1", "x86_64"),
            ("Rustday_Edit_X64\nOK", "0", "x86_64"),
            ("Rustday_Edit_X64\nOK", "1", "arm64-v8a"),
        ] {
            assert!(validate_observations(name, boot, abi).is_err());
        }
    }
}
