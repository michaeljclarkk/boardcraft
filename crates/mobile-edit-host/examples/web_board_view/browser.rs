use std::process::{Command, Stdio};

/// Open the board list once the fixture is ready. Keep the printed address
/// usable when the machine has no graphical browser or file association.
pub(super) fn open_board_list(url: String) {
    #[cfg(target_os = "windows")]
    let mut opener = {
        let mut cmd = Command::new("rundll32.exe");
        cmd.arg("url.dll,FileProtocolHandler");
        cmd
    };
    #[cfg(target_os = "macos")]
    let mut opener = Command::new("open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut opener = Command::new("xdg-open");
    std::thread::spawn(move || {
        if !opener
            .arg(url)
            .stdin(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
        {
            eprintln!("Could not open the default browser. Open the address printed above.");
        }
    });
}
