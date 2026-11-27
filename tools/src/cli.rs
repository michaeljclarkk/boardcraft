use crate::dev::Dev;
use anyhow::{Context, Result, bail, ensure};
use std::{
    env,
    ffi::OsString,
    path::Path,
    process::Command,
};

#[cfg(not(windows))]
const URL: &str = "RUSTDAY_TEST_DATABASE_URL";
const USAGE: &str = "boardcraft-tools port-status [--check|--summary] | prepare-web | dev -- PROGRAM [ARG...] | postgres start|stop|exec -- PROGRAM [ARG...] | viewer [--for-seconds N]";

pub(crate) fn run() -> Result<()> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("missing checkout root")?;
    if args == ["--help"] || args == ["-h"] {
        println!("{USAGE}");
        return Ok(());
    }
    let dev = Dev::load(root)?;
    match args.as_slice() {
        [op] if op == "port-status" => {
            crate::port_status::run(root, crate::port_status::Mode::Write)
        }
        [op, mode] if op == "port-status" && mode == "--check" => {
            crate::port_status::run(root, crate::port_status::Mode::Check)
        }
        [op, mode] if op == "port-status" && mode == "--summary" => {
            crate::port_status::run(root, crate::port_status::Mode::Summary)
        }
        [op] if op == "prepare-web" => crate::web_platform::prepare(&dev),
        [op, separator, bin, args @ ..] if op == "dev" && separator == "--" => {
            crate::web_platform::prepare(&dev)?;
            launch(dev.command(bin)?.args(args))
        }
        [op, args @ ..] if op == "postgres" => database(&dev, args),
        [op, args @ ..] if op == "viewer" => viewer(&dev, args),
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

fn viewer(dev: &Dev, args: &[OsString]) -> Result<()> {
    crate::viewer_lifetime::duration(args).map_err(anyhow::Error::msg)?;
    crate::web_platform::prepare(dev)?;
    let mut child = dev.command("cargo")?;
    #[cfg(windows)]
    crate::postgres::scope(&mut child, &crate::postgres::start(dev)?);
    #[cfg(not(windows))]
    child.env(
        URL,
        env::var_os(URL).context("Configure the disposable database URL")?,
    );
    child.args([
        "run",
        "--locked",
        "-p",
        "boardcraft-mobile-edit-host",
        "--example",
        "web_board_view",
        "--",
    ]);
    launch(child.args(args))
}
