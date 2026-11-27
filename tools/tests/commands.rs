use std::process::Command;

fn tools() -> Command {
    Command::new(env!("CARGO_BIN_EXE_boardcraft-tools"))
}

#[test]
fn dev_passes_literal_arguments_to_the_executable() {
    let value = "two words & (literal); $value \"quoted\"";
    let cfg = format!("boardcraft.fixture={value}");
    let output = tools()
        .args([
            "dev",
            "--",
            "git",
            "-c",
            &cfg,
            "config",
            "--get",
            "boardcraft.fixture",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim_end(), value);
}

#[test]
fn retired_queue_commands_are_rejected() {
    for args in [
        &["run", "1"][..],
        &["next", "1"],
        &["status"],
    ] {
        assert!(
            !tools().args(args).output().unwrap().status.success(),
            "{args:?}"
        );
    }
}

#[test]
fn invalid_viewer_deadlines_fail_before_starting_a_fixture() {
    for args in [
        &["--for-seconds"][..],
        &["--for-seconds", "0"],
        &["--for-seconds", "901"],
        &["--for-seconds", "+1"],
        &["--for-seconds", "1.5"],
        &["--for-seconds", "1", "extra"],
    ] {
        let output = tools().arg("viewer").args(args).output().unwrap();
        assert!(!output.status.success(), "{args:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("--for-seconds"));
        assert!(output.stdout.is_empty());
    }
}
