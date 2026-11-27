mod cli;
mod dev;
mod port_status;
#[cfg(windows)]
mod postgres;
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
