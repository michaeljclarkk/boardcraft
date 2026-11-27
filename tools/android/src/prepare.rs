use crate::process::{SHORT, checked, text};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Path, process::Command, time::Duration};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Provenance {
    repository: String,
    revision: String,
    patch_sha256: String,
    patched_tree: String,
}

fn git(path: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command.args(["-C"]).arg(path).args(args);
    command
}

fn assert_source(path: &Path, source: &Provenance) -> Result<()> {
    for (args, expected) in [
        (vec!["rev-parse", "HEAD"], &source.revision),
        (vec!["remote", "get-url", "origin"], &source.repository),
        (vec!["write-tree"], &source.patched_tree),
    ] {
        ensure!(
            text(&mut git(path, &args), SHORT)?.trim() == expected,
            "unexpected mobile source provenance; preserve the existing dependency directory"
        );
    }
    checked(&mut git(path, &["diff", "--quiet", "--no-ext-diff"]), SHORT)?;
    ensure!(
        text(&mut git(path, &["ls-files", "--others"]), SHORT)?
            .trim()
            .is_empty(),
        "unexpected untracked mobile source files"
    );
    Ok(())
}

pub(crate) fn prepare(android: &Path) -> Result<()> {
    let source: Provenance =
        serde_json::from_slice(&fs::read(android.join("patches/gpui-mobile.json"))?)?;
    let patch = android.join("patches/gpui-mobile-kit-011.patch");
    ensure!(
        format!("{:x}", Sha256::digest(fs::read(&patch)?))
            .eq_ignore_ascii_case(&source.patch_sha256),
        "mobile compatibility diff checksum mismatch"
    );
    let destination = android.join(".deps/gpui-mobile");
    if destination.exists() {
        return assert_source(&destination, &source);
    }
    fs::create_dir_all(android.join(".deps"))?;
    // create_dir is exclusive: a concurrent preparation cannot become our disposable work.
    fs::create_dir(&destination)?;
    let result = (|| {
        for args in [
            vec!["init", "--quiet"],
            vec!["config", "core.autocrlf", "false"],
            vec!["config", "core.eol", "lf"],
            vec!["remote", "add", "origin", &source.repository],
        ] {
            checked(&mut git(&destination, &args), SHORT)?;
        }
        checked(
            &mut git(
                &destination,
                &["fetch", "--quiet", "--depth=1", "origin", &source.revision],
            ),
            Duration::from_secs(180),
        )?;
        checked(
            &mut git(
                &destination,
                &["checkout", "--quiet", "--detach", "FETCH_HEAD"],
            ),
            SHORT,
        )?;
        ensure!(
            text(&mut git(&destination, &["rev-parse", "HEAD"]), SHORT)?.trim() == source.revision,
            "unexpected fetched mobile revision"
        );
        for check in [true, false] {
            let mut command = git(
                &destination,
                &["apply", "--index", "--unidiff-zero", "--whitespace=error"],
            );
            if check {
                command.arg("--check");
            }
            checked(command.arg(dunce::canonicalize(&patch)?), SHORT)?;
        }
        assert_source(&destination, &source)
    })();
    if result.is_err() {
        remove_owned_tree(&destination).context("clean only this failed preparation")?;
    }
    result
}

fn remove_owned_tree(path: &Path) -> Result<()> {
    // Git objects can be read-only on Windows. Never call this on a reused checkout.
    for item in fs::read_dir(path)? {
        let item = item?;
        if item.file_type()?.is_dir() {
            remove_owned_tree(&item.path())?;
        } else {
            #[cfg(windows)]
            {
                let mut permissions = item.metadata()?.permissions();
                if permissions.readonly() {
                    #[expect(
                        clippy::permissions_set_readonly_false,
                        reason = "Windows-only: clear Git's read-only attribute on this invocation's disposable files"
                    )]
                    permissions.set_readonly(false);
                    fs::set_permissions(item.path(), permissions)?;
                }
            }
            fs::remove_file(item.path())?;
        }
    }
    fs::remove_dir(path)?;
    Ok(())
}

pub(crate) fn verify_preparation(android: &Path) -> Result<()> {
    let work = tempfile::Builder::new()
        .prefix("boardcraft-mobile-source-")
        .tempdir_in(temp_root())?;
    let patches = work.path().join("patches");
    fs::create_dir(&patches)?;
    for name in ["gpui-mobile.json", "gpui-mobile-kit-011.patch"] {
        fs::copy(android.join("patches").join(name), patches.join(name))?;
    }
    let result = preparation_cases(work.path());
    // TempDir's Windows cleanup already handles read-only files.
    work.close()?;
    result
}

fn preparation_cases(android: &Path) -> Result<()> {
    prepare(android)?;
    let host = android.join(".deps/gpui-mobile");
    let manifest = host.join("Cargo.toml");
    let original = fs::read(&manifest)?;
    prepare(android)?;
    ensure!(
        fs::read(&manifest)? == original,
        "replay changed mobile source"
    );
    println!("PASS: fresh public preparation and unchanged replay");
    fs::OpenOptions::new()
        .append(true)
        .open(&manifest)?
        .write_all(b"\n# unexpected change\n")?;
    ensure!(prepare(android).is_err(), "modified worktree accepted");
    println!("PASS: refused a modified worktree");
    checked(&mut git(&host, &["add", "Cargo.toml"]), SHORT)?;
    ensure!(prepare(android).is_err(), "modified index accepted");
    println!("PASS: refused a modified index/tree");
    fs::write(&manifest, original)?;
    checked(&mut git(&host, &["add", "Cargo.toml"]), SHORT)?;
    fs::write(host.join("unexpected.rs"), b"// synthetic mutation\n")?;
    ensure!(prepare(android).is_err(), "untracked source accepted");
    fs::remove_file(host.join("unexpected.rs"))?;
    prepare(android)?;
    println!("PASS: refused unexpected files; intact source reusable");
    let patch = android.join("patches/gpui-mobile-kit-011.patch");
    fs::OpenOptions::new()
        .append(true)
        .open(&patch)?
        .write_all(b"\nchanged recipe\n")?;
    ensure!(prepare(android).is_err(), "changed recipe accepted");
    println!("PASS: changed recipe refused before source reuse");
    fs::rename(&host, android.join("preserved-source"))?;
    fs::write(&patch, b"not a patch")?;
    let provenance = android.join("patches/gpui-mobile.json");
    let mut source: Provenance = serde_json::from_slice(&fs::read(&provenance)?)?;
    source.patch_sha256 = format!("{:x}", Sha256::digest(fs::read(&patch)?));
    fs::write(provenance, serde_json::to_vec(&source)?)?;
    ensure!(prepare(android).is_err(), "invalid patch accepted");
    ensure!(
        !host.exists() && fs::read_dir(android.join(".deps"))?.next().is_none(),
        "failed preparation left an active or partial source"
    );
    println!("PASS: incompatible patch failed closed; only new checkout cleaned");
    Ok(())
}

pub(crate) fn temp_root() -> std::path::PathBuf {
    #[cfg(windows)]
    {
        std::path::PathBuf::from(r"C:\Windows\Temp\opencode")
    }
    #[cfg(not(windows))]
    {
        std::env::temp_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recipe_mismatch_never_touches_existing_dependency() {
        let work = tempfile::tempdir().unwrap();
        fs::create_dir(work.path().join("patches")).unwrap();
        fs::create_dir_all(work.path().join(".deps/gpui-mobile")).unwrap();
        fs::write(work.path().join(".deps/gpui-mobile/keep"), b"keep").unwrap();
        fs::write(
            work.path().join("patches/gpui-mobile-kit-011.patch"),
            b"wrong",
        )
        .unwrap();
        fs::write(
            work.path().join("patches/gpui-mobile.json"),
            br#"{"repository":"x","revision":"x","patchedTree":"x","patchSha256":"x"}"#,
        )
        .unwrap();
        assert!(prepare(work.path()).is_err());
        assert_eq!(
            fs::read(work.path().join(".deps/gpui-mobile/keep")).unwrap(),
            b"keep"
        );
    }
}
