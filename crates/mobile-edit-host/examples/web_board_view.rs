//! Build and serve the real wasm client for a human using an ordinary browser.
#[path = "web_board_view/browser.rs"]
mod browser;
#[allow(dead_code)]
#[path = "../tests/fixture/mod.rs"]
mod fixture;
#[path = "web_board_view/lifetime.rs"]
mod lifetime;

use std::{io, path::PathBuf, process::Command};

use boardcraft_board::{Board, BoardId, ItemId, store};
use boardcraft_mobile_edit_host::{Host, REVIEW_OWNER};
use postgres::NoTls;
use uuid::Uuid;

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
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args == ["--help"] {
        println!("web_board_view [--for-seconds N]: build wasm and serve a disposable board list.");
        println!(
            "Default: open the browser and serve until Enter. Timed: print the address and serve for 1 to 900 seconds."
        );
        return Ok(());
    }
    let duration = lifetime::duration(&args)?;
    let web = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web");
    println!("Building the locked wasm client…");
    let status = Command::new("trunk")
        .current_dir(&web)
        .env_remove("RUSTUP_TOOLCHAIN")
        .args(["build", "--locked"])
        .status()
        .map_err(|_| "Cannot start Trunk; install the pinned web toolchain first.")?;
    if !status.success() {
        return Err("Wasm build failed; no viewer was started.");
    }
    let schema = fixture::Schema::new();
    let board_id = BoardId::new(Uuid::new_v4()).ok_or("Cannot allocate board identity.")?;
    let host = Host::start_disposable_viewer(&schema.config, board_id, &web.join("dist"))
        .map_err(|_| "Cannot start the loopback viewer.")?;
    let mut db = schema
        .config
        .connect(NoTls)
        .map_err(|_| "Cannot seed viewer database.")?;
    seed_review_board(&mut db, board_id, REVIEW_OWNER)?;
    let backlog = BoardId::new(Uuid::new_v4()).ok_or("Cannot allocate board identity.")?;
    store::create_board(&mut db, &Board::new(backlog)).map_err(|_| "Cannot seed Backlog.")?;
    let workspace = Uuid::new_v4();
    let mut tx = db.transaction().map_err(|_| "Cannot seed viewer access.")?;
    tx.execute(
        "INSERT INTO workspaces(id,creator_id,name,position,active,is_open) VALUES ($1,$2,'Review',0,true,false)",
        &[&workspace, &REVIEW_OWNER],
    ).map_err(|_| "Cannot seed viewer access.")?;
    tx.execute(
        "INSERT INTO board_policies(board_id,workspace_id,name,position,kind,creator_id,active) VALUES ($1,$2,'Review',0,'private',$3,true), ($4,$2,'Backlog',1,'private',$3,true)",
        &[&board_id.as_uuid(), &workspace, &REVIEW_OWNER, &backlog.as_uuid()],
    ).map_err(|_| "Cannot seed viewer access.")?;
    tx.commit().map_err(|_| "Cannot commit viewer access.")?;
    let url = format!("http://127.0.0.1:{}/web/", host.port);
    println!("Open in your normal browser: {url}");
    println!("Choose Review for the rename journey, or Backlog for an empty board.");
    println!(
        "Review has two Work groups: the first has one Review, the second has Review and Sketch. Reserve starts empty."
    );
    if let Some(duration) = duration {
        println!("Serving for {} seconds before cleanup.", duration.as_secs());
        std::thread::sleep(duration);
    } else {
        browser::open_board_list(url);
        println!("Press Enter here to stop this viewer and remove its synthetic data.");
        if io::stdin()
            .read_line(&mut String::new())
            .map_err(|_| "Cannot read viewer console.")?
            == 0
        {
            return Err("Viewer input closed; stopping this viewer.");
        }
    }
    drop(host);
    drop(db);
    drop(schema);
    Ok(())
}

// Keep both original Review IDs, but make group identity, per-group accents,
// same-group reorder and an empty destination observable on the first visit.
fn seed_review_board(
    db: &mut postgres::Client,
    board: BoardId,
    owner: Uuid,
) -> Result<(), &'static str> {
    let second_group = "g-00000000-0000-0000-0000-000000000002";
    let empty_group = "g-00000000-0000-0000-0000-000000000003";
    store::create_board(db, &Board::new(board)).map_err(|_| "Cannot seed viewer board.")?;
    let mut tx = db.transaction().map_err(|_| "Cannot start viewer seed.")?;
    store::edit_group(&mut tx, board, "main", 1, 1, "Work", "#579bfc")
        .map_err(|_| "Cannot seed first group.")?;
    store::create_group(&mut tx, board, second_group, 2, "Work", "#00c875")
        .map_err(|_| "Cannot seed second group.")?;
    store::create_group(&mut tx, board, empty_group, 3, "Reserve", "#443322")
        .map_err(|_| "Cannot seed empty group.")?;
    for (group, id, title) in [
        ("main", fixture::FIRST, "Review"),
        (second_group, fixture::SECOND, "Review"),
        (second_group, Uuid::from_u128(63), "Sketch"),
    ] {
        let id = ItemId::new(id).ok_or("Invalid fixture identity.")?;
        store::create_group_item(&mut tx, board, group, id, owner, title)
            .map_err(|_| "Cannot seed viewer item.")?;
    }
    tx.commit().map_err(|_| "Cannot commit viewer groups.")
}
