use anyhow::{Context, Result, bail, ensure};
use std::{
    ffi::{OsStr, OsString},
    fs::{File, OpenOptions},
    io::Read,
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

pub(crate) const SHORT: Duration = Duration::from_secs(30);
const MAX_OUTPUT: u64 = 16 * 1024 * 1024;

pub(crate) struct Running {
    child: Child,
}

impl Running {
    pub(crate) fn spawn(command: &mut Command) -> Result<Self> {
        Ok(Self {
            child: command.spawn().context("start external tool")?,
        })
    }

    pub(crate) fn child(&mut self) -> &mut Child {
        &mut self.child
    }

    pub(crate) fn wait(&mut self, timeout: Duration) -> Result<ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.child.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                self.kill()?;
                bail!(
                    "external tool timed out after {} seconds",
                    timeout.as_secs()
                );
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn kill(&mut self) -> Result<()> {
        if self.child.try_wait()?.is_none() {
            // Target only this invocation's child process and descendants (SDK .bat launchers).
            #[cfg(windows)]
            {
                let status = Command::new("taskkill.exe")
                    .args(["/PID", &self.child.id().to_string(), "/T", "/F"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()?;
                if !status.success() && self.child.try_wait()?.is_none() {
                    self.child.kill()?;
                }
            }
            #[cfg(not(windows))]
            self.child.kill()?;
            self.child.wait()?;
        }
        Ok(())
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

fn drain(input: impl Read + Send + 'static) -> Receiver<std::io::Result<Vec<u8>>> {
    let (send, recv) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = input
            .take(MAX_OUTPUT + 1)
            .read_to_end(&mut bytes)
            .and_then(|_| {
                if bytes.len() as u64 > MAX_OUTPUT {
                    Err(std::io::Error::other("external output exceeds 16 MiB"))
                } else {
                    Ok(bytes)
                }
            });
        let _ = send.send(result);
    });
    recv
}

pub(crate) fn capture(command: &mut Command, timeout: Duration) -> Result<Vec<u8>> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut process = Running::spawn(command)?;
    let stdout = drain(process.child().stdout.take().context("capture stdout")?);
    let stderr = drain(process.child().stderr.take().context("capture stderr")?);
    let status = process.wait(timeout)?;
    // A descendant retaining a pipe must not hang the operator command.
    let out = stdout
        .recv_timeout(SHORT)
        .context("stdout pipe did not close")??;
    let err = stderr
        .recv_timeout(SHORT)
        .context("stderr pipe did not close")??;
    ensure!(
        status.success(),
        "external tool failed ({status}): {}",
        String::from_utf8_lossy(&err)
    );
    Ok(out)
}

pub(crate) fn text(command: &mut Command, timeout: Duration) -> Result<String> {
    String::from_utf8(capture(command, timeout)?).context("tool returned non-UTF-8 output")
}

pub(crate) fn checked(command: &mut Command, timeout: Duration) -> Result<()> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    ensure!(
        Running::spawn(command)?.wait(timeout)?.success(),
        "external tool failed"
    );
    Ok(())
}

pub(crate) fn cargo(root: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root).args(args);
    cmd
}

/// Shared lock location for cooperating workers; released by the OS after crashes.
pub(crate) fn build_lock(root: &Path) -> Result<File> {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .unwrap_or_else(|| root.join("target").into_os_string());
    std::fs::create_dir_all(&target)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(Path::new(&target).join(".boardcraft-build.lock"))?;
    lock.try_lock()
        .context("another cooperating worker owns the build lease; retry after its build")?;
    Ok(lock)
}

pub(crate) fn tool(home: &str, tail: &str, fallback: &str) -> OsString {
    std::env::var_os(home)
        .map(|p| Path::new(&p).join(tail).into_os_string())
        .unwrap_or_else(|| OsStr::new(fallback).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_and_hung_children_fail_closed() {
        #[cfg(windows)]
        {
            assert!(capture(Command::new("cmd").args(["/D", "/C", "exit 7"]), SHORT,).is_err());
            assert!(
                capture(
                    Command::new("ping").args(["-n", "30", "127.0.0.1"]),
                    Duration::from_millis(100),
                )
                .is_err()
            );
        }
    }
}
