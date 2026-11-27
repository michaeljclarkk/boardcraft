mod cli;
mod dev;
mod gates;
mod packets;
#[cfg(windows)]
mod postgres;
mod process;
mod queue;
mod web_platform;

fn main() -> std::process::ExitCode {
    match cli::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
