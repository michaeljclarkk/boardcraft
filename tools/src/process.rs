use anyhow::{Context, Result, bail, ensure};
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use wait_timeout::ChildExt;

pub(crate) const DEFAULT: Duration = Duration::from_secs(2 * 60 * 60);

/// Never format the command itself: environments and arguments can be private.
pub(crate) fn checked(cmd: &mut Command, capture: bool, timeout: Duration) -> Result<String> {
    let name = cmd.get_program().to_string_lossy().into_owned();
    let start = Instant::now();
    cmd.stdin(Stdio::null());
    if capture {
        cmd.stdout(Stdio::piped());
    }
    let mut child = cmd.spawn().with_context(|| format!("start {name}"))?;
    let output = child.stdout.take().map(|mut pipe| {
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = pipe.read_to_end(&mut bytes).map(|_| bytes);
            let _ = tx.send(result);
        });
        rx
    });
    let status = match child.wait_timeout(timeout) {
        Ok(Some(status)) => status,
        result => {
            let _ = child.kill();
            child.wait().context("reap child after interrupted wait")?;
            result?;
            bail!("{name} timed out after {} seconds", timeout.as_secs());
        }
    };
    let text = if let Some(rx) = output {
        let bytes = rx
            .recv_timeout(timeout.saturating_sub(start.elapsed()))
            .context("child stdout did not close")??;
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        String::new()
    };
    if capture {
        print!("{text}");
    }
    ensure!(status.success(), "{name} stopped ({status})");
    Ok(text)
}
