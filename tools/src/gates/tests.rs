use super::*;
use crate::packets::packets;
use std::path::Path;

const SERIAL: &str = "emulator-5582";
fn probe_reply(args: &[&str]) -> String {
    match args {
        ["-s", SERIAL, "emu", "avd", "name"] => "Rustday_Edit_X64\r\r\nOK\r\r\n",
        ["-s", SERIAL, "shell", "getprop", "sys.boot_completed"] => "1",
        ["-s", SERIAL, "shell", "getprop", "ro.product.cpu.abi"] => "x86_64",
        _ => panic!("unexpected adb command {args:?}"),
    }
    .to_owned()
}

#[test]
fn device_probes_refuse_non_emulators_and_wrong_responses_before_mutation() {
    for bad in [
        "",
        "192.0.2.1:5555",
        "usb-phone",
        "emulator-1 extra",
        "emulator-",
    ] {
        check_probe(
            &mut |_, _, _, _| panic!("no adb before serial validation"),
            bad,
        )
        .unwrap_err();
    }
    for (index, bad, message) in [
        (0, "another_avd\nOK", "Rustday_Edit_X64"),
        (1, "0", "not booted"),
        (2, "arm64-v8a", "x86_64"),
    ] {
        let mut probe = 0;
        let err = check_probe(
            &mut |bin, args, capture, timeout| {
                assert_eq!(bin, "adb");
                assert!(capture);
                assert_eq!(timeout, Duration::from_secs(10));
                let good = probe_reply(args);
                probe += 1;
                Ok(if probe == index + 1 { bad.into() } else { good })
            },
            SERIAL,
        )
        .unwrap_err();
        assert!(err.to_string().contains(message));
        assert_eq!(probe, index + 1);
    }
}

#[test]
fn every_brief_uses_the_dedicated_worker_and_only_board_is_db_free() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../packets");
    for p in packets() {
        assert!(root.join(format!("{}.md", p.id)).is_file(), "{}", p.id);
        assert_eq!(p.model, "openai/gpt-6.1-sol#xhigh");
        assert_eq!(p.database, p.id != "01-board");
    }
}

// Inject evidence at the process boundary. A near-match or a case passed in a
// *different* suite is not evidence for the required suite.
fn suites(p: &Packet) -> Vec<(&str, &[&str])> {
    let mut suites: Vec<(&str, &[&str])> = vec![];
    if !p.windows.is_empty() {
        suites.push(("windows_smoke", &p.windows));
    }
    if p.board_link {
        suites.push(("board address", &[LINK]));
    }
    if let Some(target) = p.target {
        suites.push((target, &p.cases));
    }
    if matches!(p.device, Device::Restore | Device::Logout | Device::SignIn) {
        suites.push((PROBE, PROBE_CASES));
    }
    suites
}

type Injection<'a> = Option<(&'a str, Option<&'a str>)>;
fn trial(p: &Packet, bad: Injection, fail: Option<&str>) -> Result<Vec<String>> {
    simulate(p, bad, fail, |_, _| {})
}

fn simulate(
    p: &Packet,
    bad: Injection,
    fail: Option<&str>,
    mut observe: impl FnMut(&str, &[&str]),
) -> Result<Vec<String>> {
    let expected = suites(p);
    let mut calls = vec![];
    let result = verify(
        p,
        &mut |bin, args, capture, timeout| {
            observe(bin, args);
            if bin == "adb" {
                return Ok(probe_reply(args));
            }
            assert_eq!(bin, "cargo");
            let target = args.windows(2).find(|a| a[0] == "--test").map(|a| a[1]);
            let target = if args.contains(&"--exact") {
                Some("board address")
            } else {
                target
            };
            let name = if let Some(n) = target {
                n.to_owned()
            } else if let Some(i) = args.iter().position(|a| *a == "--") {
                args[i + 1..].join(" ")
            } else {
                args.join(" ")
            };
            calls.push(name.clone());
            ensure!(Some(name.as_str()) != fail, "failed {name}");
            if !capture {
                return Ok(String::new());
            }
            let name = target.expect("capture only named suites");
            if name == PROBE {
                assert_eq!(timeout, Duration::from_secs(600));
            }
            let cases = expected
                .iter()
                .find(|(n, _)| *n == name)
                .expect("unexpected suite")
                .1;
            let mut out = String::new();
            for case in cases {
                let suffix = if bad == Some((name, Some(*case))) {
                    "_other"
                } else {
                    ""
                };
                out.push_str(&format!("test {case}{suffix} ... ok\r\n"));
            }
            if let Some((wrong, Some(case))) = bad
                && wrong != name
            {
                out.push_str(&format!("test {case} ... ok\r\n"));
            }
            let ignored = usize::from(bad == Some((name, None)));
            out.push_str(&format!(
                "test result: ok. {} passed; 0 failed; {ignored} ignored; 0 measured",
                cases.len().max(1)
            ));
            Ok(out)
        },
        SERIAL,
    );
    if bad.is_none() && fail.is_none() && !p.pending_ui.is_empty() {
        let error = result.expect_err("unobserved live journey must not verify the packet");
        assert!(
            error
                .to_string()
                .contains("pending Computer Use observation")
        );
    } else {
        result?;
    }
    Ok(calls)
}

// Independent accepted inventory captured at cutover. Removing a case from
// both the packet and its simulated stdout must not weaken the regression.
#[test]
fn preserves_required_commands_cases_and_pending_outcomes_at_cutover() {
    use std::collections::BTreeSet;
    for (i, p) in packets().iter().take(56).enumerate() {
        let id = i + 1;
        let mut actual = BTreeSet::new();
        simulate(p, None, None, |bin, args| {
            actual.insert(format!("{bin} {}", args.join(" ")));
        })
        .unwrap();
        for (name, cases) in suites(p) {
            actual.extend(cases.iter().map(|case| format!("{name}::{case}")));
        }
        actual.extend(p.pending_ui.iter().map(|case| format!("pending::{case}")));
        let expected = include_str!("../../tests/cutover-gates.tsv")
            .lines()
            .filter_map(|row| {
                let (ranges, gate) = row.split_once('\t').unwrap();
                ranges
                    .split(',')
                    .any(|r| {
                        let (lo, hi) = r.split_once('-').unwrap_or((r, r));
                        (lo.parse::<usize>().unwrap()..=hi.parse::<usize>().unwrap()).contains(&id)
                    })
                    .then(|| gate.to_owned())
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "{} changed its acceptance gates", p.id);
    }
}

#[test]
fn each_required_case_must_pass_in_its_own_enabled_suite() {
    for p in packets() {
        let calls = trial(&p, None, None).unwrap();
        for (suite, names) in suites(&p) {
            assert_eq!(
                calls.iter().filter(|n| *n == suite).count(),
                1,
                "{}: {suite}",
                p.id
            );
            for &name in names {
                let e = trial(&p, Some((suite, Some(name))), None).unwrap_err();
                assert_eq!(
                    e.to_string(),
                    format!("Missing passing {suite} case: {name}")
                );
            }
            assert_eq!(
                trial(&p, Some((suite, None)), None)
                    .unwrap_err()
                    .to_string(),
                format!("No passing, enabled {suite} suite")
            );
        }
        // A failing delivery, build or regression command must stop verification.
        for call in calls {
            assert!(trial(&p, None, Some(&call)).is_err());
        }
    }
    for bad in [
        "",
        "test result: ok. 0 passed; 0 failed; 0 ignored",
        "test result: ok. 1 passed; 1 failed; 0 ignored",
        "test result: ok. 1 passed; 0 failed; 10 ignored",
    ] {
        assert!(require_cases("fixture", &[], bad).is_err());
    }
}
