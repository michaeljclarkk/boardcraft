mod commands;
mod dev;
mod port_status;
#[cfg(windows)]
mod postgres;
mod viewer_lifetime;
mod web_platform;

fn main() -> std::process::ExitCode {
    match commands::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
