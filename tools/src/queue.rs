use crate::packets::Packet;
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub(crate) fn limit(raw: &str, total: usize) -> Result<usize> {
    let n = raw
        .parse::<usize>()
        .ok()
        .filter(|n| (1..=total).contains(n) && n.to_string() == raw);
    n.with_context(|| format!("Expected 1-{total} packets"))
}

fn state(root: &Path) -> Result<PathBuf> {
    let legacy = root.join(".rustday");
    let new = root.join(".boardcraft");
    if legacy.try_exists()? {
        ensure!(
            !new.try_exists()?,
            "Both .rustday and .boardcraft runner states exist; resolve the conflict before continuing."
        );
        Ok(legacy)
    } else {
        Ok(new)
    }
}

pub(crate) fn status(root: &Path, packets: &[Packet]) -> Result<()> {
    let dir = state(root)?;
    for p in packets {
        let verified = dir.join(format!("{}.ok", p.id)).try_exists()?;
        println!("{} {}", p.id, if verified { "verified" } else { "pending" });
    }
    Ok(())
}

#[derive(Deserialize)]
struct Handoff {
    status: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    next: String,
    #[serde(default)]
    evidence: Vec<String>,
}
fn accept(path: &Path, id: &str) -> Result<()> {
    let file = fs::File::open(path).with_context(|| format!("Missing fresh handoff for {id}"))?;
    let h: Handoff =
        serde_json::from_reader(file).with_context(|| format!("Incomplete handoffs/{id}.json"))?;
    ensure!(
        h.status == "ready",
        "{id}: {}",
        if h.summary.is_empty() {
            "incomplete"
        } else {
            &h.summary
        }
    );
    ensure!(
        !h.summary.trim().is_empty()
            && !h.next.trim().is_empty()
            && !h.evidence.is_empty()
            && h.evidence.iter().all(|s| !s.trim().is_empty()),
        "Incomplete handoffs/{id}.json"
    );
    Ok(())
}

pub(crate) fn run(
    root: &Path,
    packets: &[Packet],
    mut count: usize,
    mut work: impl FnMut(&Packet) -> Result<()>,
    mut check: impl FnMut(&Packet) -> Result<()>,
) -> Result<()> {
    let dir = state(root)?;
    fs::create_dir_all(&dir)?;
    let handoffs = root.join("handoffs");
    fs::create_dir_all(&handoffs)?;
    let lock = dir.join("lock");
    let mut owner = fs::File::create_new(&lock)
        .with_context(|| format!("Could not acquire runner lock {}", lock.display()))?;
    let result = (|| -> Result<()> {
        writeln!(owner, "{}", std::process::id())?;
        for p in packets {
            if count == 0 {
                break;
            }
            let id = p.id;
            let marker = dir.join(format!("{id}.ok"));
            if marker.try_exists()? {
                continue;
            }
            let handoff = handoffs.join(format!("{id}.json"));
            if handoff.try_exists()? {
                fs::copy(&handoff, dir.join(format!("{id}.previous.json")))?;
                fs::remove_file(&handoff)?;
            }
            work(p)?;
            accept(&handoff, id)?;
            check(p)?;
            let mut stamp = tempfile::NamedTempFile::new_in(&dir)?;
            write!(
                stamp,
                "{}",
                chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            )?;
            stamp.persist_noclobber(marker)?;
            println!("Verified {id}");
            count -= 1;
        }
        Ok(())
    })();
    drop(owner);
    fs::remove_file(lock).context("release runner lock")?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packets::packets;
    const READY: &str =
        r#"{"status":"ready","summary":"fixture","next":"continue","evidence":["checked"]}"#;
    #[test]
    fn resume_requires_a_fresh_ready_handoff_and_successful_gates() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        let ps = packets();
        let work = |p: &Packet| {
            fs::write(root.join(format!("handoffs/{}.json", p.id)), READY)?;
            Ok(())
        };
        let mut seen = vec![];
        run(root, &ps, 3, work, |p| {
            seen.push(p.id);
            ensure!(p.id != "02-postgres", "failed gate");
            Ok(())
        })
        .unwrap_err();
        assert_eq!(seen, ["01-board", "02-postgres"]);
        let dir = root.join(".boardcraft");
        assert!(dir.join("01-board.ok").exists());
        assert!(!dir.join("02-postgres.ok").exists());
        let err = run(
            root,
            &ps,
            1,
            |_| Ok(()),
            |_| panic!("stale handoff accepted"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("Missing fresh handoff"));
        assert_eq!(
            fs::read_to_string(dir.join("02-postgres.previous.json")).unwrap(),
            READY
        );
        for body in [
            "not json",
            r#"{"status":"blocked","summary":"blocked"}"#,
            r#"{"status":"ready","summary":"  ","next":"go","evidence":["ok"]}"#,
            r#"{"status":"ready","summary":"ok","next":"","evidence":["ok"]}"#,
            r#"{"status":"ready","summary":"ok","next":"go","evidence":[]}"#,
            r#"{"status":"ready","summary":"ok","next":"go","evidence":[" "]}"#,
        ] {
            run(
                root,
                &ps,
                1,
                |_| Ok(fs::write(root.join("handoffs/02-postgres.json"), body)?),
                |_| panic!("incomplete handoff accepted"),
            )
            .unwrap_err();
            assert!(!dir.join("02-postgres.ok").exists());
        }
        run(root, &ps, 1, work, |_| Ok(())).unwrap();
        let mut seen = vec![];
        run(root, &ps, 2, work, |p| {
            seen.push(p.target.unwrap());
            Ok(())
        })
        .unwrap();
        assert_eq!(seen, ["desktop_edit", "mobile_edit_service"]);
        assert!(!dir.join("lock").exists());
    }
    #[test]
    fn existing_state_honors_foreign_lock_and_conflicts_fail_closed() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        let ps = packets();
        let dir = root.join(".rustday");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("01-board.ok"), "verified").unwrap();
        fs::write(dir.join("lock"), "other").unwrap();
        let refuse = |_: &Packet| -> Result<()> { panic!("must not dispatch") };
        run(root, &ps, 1, refuse, refuse).unwrap_err();
        assert_eq!(fs::read_to_string(dir.join("lock")).unwrap(), "other");
        fs::remove_file(dir.join("lock")).unwrap();
        run(
            root,
            &ps,
            1,
            |p| {
                assert_eq!(p.id, "02-postgres");
                Ok(fs::write(root.join("handoffs/02-postgres.json"), READY)?)
            },
            |_| Ok(()),
        )
        .unwrap();
        assert!(dir.join("02-postgres.ok").exists());
        assert!(!root.join(".boardcraft").exists());
        fs::create_dir(root.join(".boardcraft")).unwrap();
        assert!(
            run(root, &ps, 1, refuse, refuse)
                .unwrap_err()
                .to_string()
                .contains("Both .rustday and .boardcraft")
        );
    }
    #[test]
    fn count_is_canonical_and_within_queue() {
        let n = packets().len();
        for i in 1..=n {
            assert_eq!(limit(&i.to_string(), n).unwrap(), i);
        }
        for bad in ["", "0", "-1", "+1", "01", "1.1", "1x", &(n + 1).to_string()] {
            assert!(limit(bad, n).is_err());
        }
    }
}
