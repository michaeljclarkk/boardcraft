use crate::{
    packets::{Device, PROBE, PROBE_CASES, Packet},
    process::DEFAULT,
};
use anyhow::{Result, ensure};
use std::time::Duration;

type Exec<'a> = dyn FnMut(&str, &[&str], bool, Duration) -> Result<String> + 'a;
const DESKTOP: &str = "boardcraft-desktop";
const HOST: &str = "boardcraft-mobile-edit-host";
const SELF_TEST: &[&str] = &["test", "--locked", "--manifest-path", "tools/Cargo.toml"];
const ANDROID: &str = "tools/android/Cargo.toml";
const ANDROID_FMT: &[&str] = &[
    "fmt",
    "--manifest-path",
    "android/Cargo.toml",
    "--",
    "--check",
];
const WORKSPACE: &[&str] = &["test", "--locked", "--workspace"];
const LINK: &str = "connected::board_link::tests::only_a_canonical_board_at_the_connected_origin";

fn cargo(exec: &mut Exec, args: &[&str]) -> Result<()> {
    exec("cargo", args, false, DEFAULT).map(drop)
}

fn require_cases(suite: &str, cases: &[&str], output: &str) -> Result<()> {
    ensure!(
        output.lines().any(|line| {
            let Some((passed, rest)) = line
                .strip_prefix("test result: ok. ")
                .and_then(|s| s.split_once(" passed; "))
            else {
                return false;
            };
            passed.parse::<usize>().is_ok_and(|n| n > 0)
                && rest.split(';').take(2).eq(["0 failed", " 0 ignored"])
        }),
        "No passing, enabled {suite} suite"
    );
    for case in cases {
        let want = format!("test {case} ... ok");
        ensure!(
            output.lines().any(|line| line == want),
            "Missing passing {suite} case: {case}"
        );
    }
    Ok(())
}

fn suite(
    exec: &mut Exec,
    target: &str,
    cases: &[&str],
    args: &[&str],
    timeout: Duration,
) -> Result<()> {
    require_cases(target, cases, &exec("cargo", args, true, timeout)?)
}
fn host_cases(exec: &mut Exec, target: &str, cases: &[&str]) -> Result<()> {
    suite(
        exec,
        target,
        cases,
        &["test", "--locked", "--workspace", "--test", target],
        DEFAULT,
    )
}
fn smoke(
    exec: &mut Exec,
    package: &str,
    feature: &str,
    target: &str,
    cases: &[&str],
    timeout: Duration,
) -> Result<()> {
    let mut args = vec![
        "test",
        "--locked",
        "-p",
        package,
        "--features",
        feature,
        "--test",
        target,
    ];
    if cases.is_empty() {
        return cargo(exec, &args);
    }
    args.extend(["--", "--test-threads=1"]);
    suite(exec, target, cases, &args, timeout)
}

pub(crate) fn check_probe(exec: &mut Exec, serial: &str) -> Result<()> {
    ensure!(
        serial
            .strip_prefix("emulator-")
            .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())),
        "Set RUSTDAY_ANDROID_SERIAL to the dedicated emulator serial"
    );
    let mut adb = |args: &[&str]| {
        let mut full = vec!["-s", serial];
        full.extend_from_slice(args);
        exec("adb", &full, true, Duration::from_secs(10))
    };
    ensure!(
        adb(&["emu", "avd", "name"])?
            .trim()
            .lines()
            .next()
            .map(str::trim)
            == Some("Rustday_Edit_X64"),
        "Expected Rustday_Edit_X64 AVD"
    );
    ensure!(
        adb(&["shell", "getprop", "sys.boot_completed"])?.trim() == "1",
        "Emulator has not booted"
    );
    ensure!(
        adb(&["shell", "getprop", "ro.product.cpu.abi"])?.trim() == "x86_64",
        "Expected x86_64 Android"
    );
    Ok(())
}

pub(crate) fn preflight(device: Device, exec: &mut Exec, serial: &str) -> Result<()> {
    if device.uses_probe() {
        check_probe(exec, serial)?;
    }
    Ok(())
}

fn probe(exec: &mut Exec, serial: &str) -> Result<()> {
    check_probe(exec, serial)?;
    cargo(
        exec,
        &[
            "build",
            "--locked",
            "-p",
            "boardcraft-mobile-edit-client",
            "--bin",
            "android-edit-probe",
            "--target",
            "x86_64-linux-android",
        ],
    )?;
    smoke(
        exec,
        HOST,
        "android-smoke",
        PROBE,
        PROBE_CASES,
        Duration::from_secs(600),
    )
}

pub(crate) fn verify(p: &Packet, exec: &mut Exec, serial: &str) -> Result<()> {
    commands(p, exec, serial)?;
    ensure!(
        p.pending_ui.is_empty(),
        "{}: pending Computer Use observations: {}",
        p.id,
        p.pending_ui.join(", ")
    );
    Ok(())
}

fn commands(p: &Packet, exec: &mut Exec, serial: &str) -> Result<()> {
    cargo(exec, &["fmt", "--all", "--", "--check"])?;
    if p.wasm {
        cargo(
            exec,
            &[
                "+nightly-2026-10-31",
                "build",
                "--locked",
                "--target",
                "wasm32-unknown-unknown",
                "-p",
                "boardcraft-ui",
                "-p",
                "boardcraft-web",
            ],
        )?;
    }
    if p.board_link {
        suite(
            exec,
            "board address",
            &[LINK],
            &[
                "test", "--locked", "-p", DESKTOP, "--bin", DESKTOP, LINK, "--", "--exact",
            ],
            DEFAULT,
        )?;
        println!(
            "SKIPPED in unattended runner: two Windows clipboard cases (see packet handoff for attended evidence)."
        );
    }
    if p.device.uses_probe() {
        probe(exec, serial)?;
    }
    if p.device.needs_android_ui() {
        cargo(exec, ANDROID_FMT)?;
    }
    if p.wasm || !p.windows.is_empty() || (p.device.needs_android_ui() && p.device != Device::Frame)
    {
        regression(exec)?;
    }
    ensure!(
        p.target.is_some() || !p.windows.is_empty() || p.device.needs_android_ui(),
        "{} has no test target",
        p.id
    );
    if let Some(target) = p.target.filter(|&t| t != PROBE) {
        host_cases(exec, target, &p.cases)?;
    }
    if p.device == Device::None && p.target.is_none() && !p.windows.is_empty() {
        cargo(exec, &["test", "--locked", "-p", DESKTOP])?;
    } else {
        cargo(exec, WORKSPACE)?;
    }
    if !p.windows.is_empty()
        || matches!(
            p.device,
            Device::Delivery | Device::SignIn | Device::Logout | Device::Restore
        )
    {
        smoke(
            exec,
            DESKTOP,
            "native-smoke",
            "windows_smoke",
            &p.windows,
            DEFAULT,
        )?;
    }
    if p.bins {
        cargo(exec, &["build", "--locked", "--workspace", "--bins"])?;
    }
    Ok(())
}

fn regression(exec: &mut Exec) -> Result<()> {
    cargo(exec, SELF_TEST)?;
    cargo(exec, &["test", "--locked", "--manifest-path", ANDROID])
}

#[cfg(test)]
mod tests;
