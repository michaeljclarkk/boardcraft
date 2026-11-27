#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    match boardcraft_android_tools::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
