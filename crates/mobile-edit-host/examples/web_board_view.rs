//! Build and serve the real wasm client for a human using an ordinary browser.
#[allow(dead_code)]
#[path = "../tests/fixture/mod.rs"]
mod fixture;

use std::{io, path::PathBuf, process::Command};

use boardcraft_account::Role;
use boardcraft_board::{Board, BoardId, ItemId, store};
use postgres::NoTls;
use uuid::Uuid;

const USERNAME: &str = "BOARDCRAFT_VIEWER_USERNAME";
const PASSWORD: &str = "BOARDCRAFT_VIEWER_PASSWORD";

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), &'static str> {
    match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => (),
        [arg] if arg == "--help" => {
            println!(
                "web_board_view: build wasm, seed a disposable editable board, and serve until Enter."
            );
            println!(
                "Choose disposable credentials at the hidden console prompts, then sign in in your browser."
            );
            println!(
                "A launcher may instead supply both BOARDCRAFT_VIEWER_USERNAME and BOARDCRAFT_VIEWER_PASSWORD in the process environment. Never put them in arguments or URLs."
            );
            return Ok(());
        }
        _ => return Err("usage: web_board_view [--help]"),
    }
    let web = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web");
    println!("Building the locked wasm client…");
    let status = Command::new("trunk")
        .current_dir(&web)
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove(USERNAME)
        .env_remove(PASSWORD)
        .args(["build", "--locked"])
        .status()
        .map_err(|_| "Cannot start Trunk; install the pinned web toolchain first.")?;
    if !status.success() {
        return Err("Wasm build failed; no viewer was started.");
    }
    let (username, password) = credentials()?;
    let schema = fixture::Schema::new();
    let board_id = BoardId::new(Uuid::new_v4()).ok_or("Cannot allocate board identity.")?;
    let host =
        boardcraft_mobile_edit_host::Host::start_web(&schema.config, board_id, &web.join("dist"))
            .map_err(|_| "Cannot start the loopback web host.")?;
    let mut db = schema
        .config
        .connect(NoTls)
        .map_err(|_| "Cannot seed viewer database.")?;
    let mut board = Board::new(board_id);
    for id in [fixture::FIRST, fixture::SECOND] {
        board
            .add_item(
                ItemId::new(id).ok_or("Invalid fixture identity.")?,
                "Review",
            )
            .map_err(|_| "Cannot seed viewer board.")?;
    }
    store::create_board(&mut db, &board).map_err(|_| "Cannot seed viewer board.")?;
    let account = fixture::seed_account_on(&mut db, &username, &password, Role::Owner);
    drop(password);
    drop(username);
    let workspace = Uuid::new_v4();
    let mut tx = db.transaction().map_err(|_| "Cannot seed viewer access.")?;
    tx.execute(
        "INSERT INTO workspaces(id,creator_id,name,position,active,is_open) VALUES ($1,$2,'Review',0,true,false)",
        &[&workspace, &account],
    ).map_err(|_| "Cannot seed viewer access.")?;
    tx.execute(
        "INSERT INTO board_policies(board_id,workspace_id,name,position,kind,creator_id,active) VALUES ($1,$2,'Review',0,'private',$3,true)",
        &[&board_id.as_uuid(), &workspace, &account],
    ).map_err(|_| "Cannot seed viewer access.")?;
    tx.commit().map_err(|_| "Cannot commit viewer access.")?;
    println!(
        "Open in your normal browser: http://127.0.0.1:{}/web/boards/{}",
        host.port,
        board_id.as_uuid()
    );
    println!("Sign in with the disposable credentials you chose. The board is editable.");
    println!(
        "Keep this terminal open. Press Enter here to stop this viewer and remove its synthetic data."
    );
    if io::stdin()
        .read_line(&mut String::new())
        .map_err(|_| "Cannot read viewer console.")?
        == 0
    {
        return Err("Viewer input closed; stopping this viewer.");
    }
    drop(host);
    drop(db);
    drop(schema);
    Ok(())
}

fn credentials() -> Result<(String, String), &'static str> {
    // Process-only configuration supports protected launchers; no credential
    // file, browser argument, automatic sign-in or session bypass is created.
    let configured = (std::env::var(USERNAME), std::env::var(PASSWORD));
    let (username, password) = match configured {
        (Ok(username), Ok(password)) => (username, password),
        (Err(std::env::VarError::NotPresent), Err(std::env::VarError::NotPresent)) => (
            rpassword::prompt_password("Choose a disposable username (hidden): ")
                .map_err(|_| "A console is required for the hidden credential prompts.")?,
            rpassword::prompt_password(
                "Choose a disposable password (hidden; do not reuse a real password): ",
            )
            .map_err(|_| "Cannot read a hidden password from the console.")?,
        ),
        _ => {
            return Err(
                "Supply both viewer credential environment variables, or neither to use hidden prompts.",
            );
        }
    };
    let username = username.trim().to_owned();
    if username.is_empty() || username.len() > 128 || password.is_empty() || password.len() > 1024 {
        return Err(
            "Use a nonempty disposable username (up to 128 bytes) and password (up to 1024 bytes).",
        );
    }
    Ok((username, password))
}
