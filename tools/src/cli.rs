use crate::{
    dev::Dev,
    gates,
    packets::{self, Packet},
    process, queue,
};
use anyhow::{Context, Result, bail, ensure};
use std::{
    env,
    ffi::{OsStr, OsString},
    path::Path,
    process::Command,
};

const URL: &str = "RUSTDAY_TEST_DATABASE_URL";
const USAGE: &str = "boardcraft-tools status | run COUNT | prepare-web | dev -- PROGRAM [ARG...] | postgres start|stop|exec -- PROGRAM [ARG...] | viewer";

pub(crate) fn run() -> Result<()> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("missing checkout root")?;
    let packets = packets::packets();
    if args.is_empty() || args == ["status"] {
        return queue::status(root, &packets);
    }
    if args == ["--help"] || args == ["-h"] {
        println!("{USAGE}");
        return Ok(());
    }
    let dev = Dev::load(root)?;
    match args.as_slice() {
        [op, count] if op == "run" => dispatch(
            &dev,
            &packets,
            queue::limit(count.to_str().unwrap_or_default(), packets.len())?,
        ),
        [op] if op == "prepare-web" => crate::web_platform::prepare(&dev),
        [op, separator, bin, args @ ..] if op == "dev" && separator == "--" => {
            crate::web_platform::prepare(&dev)?;
            launch(dev.command(bin)?.args(args))
        }
        [op, args @ ..] if op == "postgres" => database(&dev, args),
        [op] if op == "viewer" => viewer(&dev),
        _ => bail!("Usage: {USAGE}"),
    }
}

fn launch(command: &mut Command) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("start {}", command.get_program().to_string_lossy()))?;
    ensure!(
        status.success(),
        "{} stopped ({status})",
        command.get_program().to_string_lossy()
    );
    Ok(())
}

#[cfg(windows)]
fn database(dev: &Dev, args: &[OsString]) -> Result<()> {
    use crate::postgres;
    match args {
        [] => postgres::start(dev).map(drop),
        [op] if op == "start" => postgres::start(dev).map(drop),
        [op] if op == "stop" => postgres::stop(dev),
        [op, sep, bin, rest @ ..] if op == "exec" && sep == "--" => {
            let mut child = dev.command(bin)?;
            crate::web_platform::prepare(dev)?;
            postgres::scope(&mut child, &postgres::start(dev)?);
            launch(child.args(rest))
        }
        _ => bail!("Usage: {USAGE}"),
    }
}

#[cfg(not(windows))]
fn database(_: &Dev, _: &[OsString]) -> Result<()> {
    bail!(
        "Local cluster management requires Windows; configure {URL} for an existing disposable database"
    )
}

fn viewer(dev: &Dev) -> Result<()> {
    // Start the ordinary viewer; live interaction belongs to Computer Use.
    crate::web_platform::prepare(dev)?;
    let mut child = dev.command("cargo")?;
    #[cfg(windows)]
    crate::postgres::scope(&mut child, &crate::postgres::start(dev)?);
    #[cfg(not(windows))]
    child.env(
        URL,
        env::var_os(URL).context("Configure the disposable database URL")?,
    );
    launch(child.args([
        "run",
        "--locked",
        "-p",
        "boardcraft-mobile-edit-host",
        "--example",
        "web_board_view",
    ]))
}

fn dispatch(dev: &Dev, packets: &[Packet], count: usize) -> Result<()> {
    crate::web_platform::prepare(dev)?;
    for bin in [OsStr::new("cargo"), &dev.cli] {
        process::checked(dev.command(bin)?.arg("--version"), false, process::DEFAULT)?;
    }
    let serial = env::var("RUSTDAY_ANDROID_SERIAL").unwrap_or_default();
    let database = env::var_os(URL).filter(|v| !v.is_empty());
    let preflight = |bin: &str, args: &[&str], capture, timeout| {
        process::checked(dev.command(bin)?.args(args), capture, timeout)
    };
    queue::run(
        &dev.root,
        packets,
        count,
        |p| {
            ensure!(
                !p.database || database.is_some(),
                "Set {URL} to the disposable boardcraft DB."
            );
            gates::preflight(p.device, &mut { preflight }, &serial)?;
            let id = p.id;
            let brief = format!("packets/{id}.md");
            ensure!(
                dev.root.join(&brief).is_file(),
                "Missing workflow brief: {brief}"
            );
            let mut worker = dev.command(&dev.cli)?;
            if let Some(url) = database.as_ref().filter(|_| p.database) {
                worker.env(URL, url);
            }
            let prompt = format!(
                "Implement {brief} under AGENTS.md. Use only Computer Use and GPUI Kit for UI verification; historical driver instructions are retired. Record the actual live observations or leave the packet blocked. Name the reference workflow or owner request. Run focused checks and update only the affected workflow row and handoffs/{id}.json for documentation. Stop after the handoff."
            );
            worker.args([
                "run",
                "--standalone",
                "--agent",
                "build",
                "--model",
                p.model,
                "--title",
                &format!("boardcraft {id}"),
                &prompt,
            ]);
            process::checked(&mut worker, false, process::DEFAULT).map(drop)
        },
        |p| {
            gates::verify(
                p,
                &mut |bin, args, capture, timeout| {
                    let mut cmd = dev.command(bin)?;
                    if let Some(url) = database
                        .as_ref()
                        .filter(|_| p.database && needs_database(bin, args))
                    {
                        cmd.env(URL, url);
                    }
                    process::checked(cmd.args(args), capture, timeout)
                },
                &serial,
            )
        },
    )
}

fn needs_database(bin: &str, args: &[&str]) -> bool {
    bin == "cargo" && args.first() == Some(&"test") && !args.contains(&"--manifest-path")
}
