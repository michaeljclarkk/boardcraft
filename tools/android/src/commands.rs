use crate::{device, package, prepare};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    about = "Prepare and package Android; device preflight is read-only and explicitly scoped"
)]
struct Cli {
    /// Checkout root (defaults to the checkout containing this crate).
    #[arg(long, global = true, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))]
    root: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Verify or prepare the exact pinned, patched mobile source.
    PrepareMobile,
    /// Run isolated public-source preparation and tamper-rejection checks.
    TestPrepareMobile,
    /// Build, package and sign; no adb or emulator actions.
    BuildDebug {
        #[arg(long)]
        output_apk: PathBuf,
    },
    /// Refuse shared/remote adb and validate the already booted dedicated AVD.
    CheckDevice,
}

/// Parse the operator command and run it, propagating prerequisite and acceptance failures.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let root = dunce::canonicalize(cli.root)?;
    match cli.command {
        Command::PrepareMobile => prepare::prepare(&root.join("android")),
        Command::TestPrepareMobile => prepare::verify_preparation(&root.join("android")),
        Command::BuildDebug { output_apk } => package::build(&root, &output_apk),
        Command::CheckDevice => device::Device::from_env()?.validate(),
    }
}
