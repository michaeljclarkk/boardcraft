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
fn bad_counts_and_unconsumed_arguments_fail_before_dispatch() {
    for args in [
        &["run", "+1"][..],
        &["run", "1", "extra"],
        &["status", "1"],
        &["dev", "--"],
        &["postgres", "exec", "tool"],
    ] {
        assert!(
            !tools().args(args).output().unwrap().status.success(),
            "{args:?}"
        );
    }
}
