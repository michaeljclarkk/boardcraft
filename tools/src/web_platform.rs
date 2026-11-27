//! Prepare the workspace path dependency without writing to Cargo's registry.
use crate::dev::Dev;
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

const SOURCE: &str = "gpui-pre-web-0.3.14";
const ARCHIVE: &str = "gpui-pre-web-0.3.14.crate";
const URL: &str = "https://static.crates.io/crates/gpui-pre-web/gpui-pre-web-0.3.14.crate";
const SHA: &str = "4c07064ebec2ae4aad179c3255fe71a5b0f83b9a757d6c767ec7b9c290f4183d";
const MARKER: &str = ".boardcraft-source";
// Preserve the existing recipe identity while both entry points produce identical trees.
const RECIPE: &str = "8F7C8385AF073742A7BB3357A45A90BEF9800A2903D56122ED6EA102D68B64CC";

fn cached(home: &Path) -> Result<Option<PathBuf>> {
    let dirs = match home.join("registry/cache").read_dir() {
        Ok(dirs) => dirs,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    for dir in dirs {
        let archive = dir?.path().join(ARCHIVE);
        if archive.is_file() {
            return Ok(Some(archive));
        }
    }
    Ok(None)
}

fn need(command: &mut Command, action: &str) -> Result<()> {
    ensure!(
        command
            .status()
            .with_context(|| action.to_owned())?
            .success(),
        "Could not {action}"
    );
    Ok(())
}

pub(crate) fn prepare(dev: &Dev) -> Result<()> {
    let deps = dev.root.join("crates/web/.deps");
    let dest = deps.join("gpui-pre-web");
    if dest.try_exists()? {
        return verify_tree(&dest);
    }
    fs::create_dir_all(&deps)?;
    let work = tempfile::Builder::new()
        .prefix("prepare-")
        .tempdir_in(&deps)?;
    let archive = match dev
        .cargo_home()
        .map(|dir| cached(&dir))
        .transpose()?
        .flatten()
    {
        Some(path) => path,
        None => {
            let path = work.path().join(ARCHIVE);
            need(
                dev.command("curl")?
                    .args([
                        "--fail",
                        "--silent",
                        "--show-error",
                        "--proto",
                        "=https",
                        "--output",
                    ])
                    .arg(&path)
                    .arg(URL),
                "download pinned public platform crate",
            )?;
            path
        }
    };
    ensure!(
        format!("{:x}", Sha256::digest(fs::read(&archive)?)) == SHA,
        "Public crate checksum mismatch"
    );
    need(
        dev.command("tar")?
            .arg("-xf")
            .arg(archive)
            .arg("-C")
            .arg(work.path()),
        "extract pinned platform crate",
    )?;
    let source = work.path().join(SOURCE);
    let patch = Path::new(env!("CARGO_MANIFEST_DIR")).join("patches/gpui-pre-web.patch");
    need(
        dev.command("git")?
            .current_dir(&source)
            .env("GIT_CEILING_DIRECTORIES", work.path())
            .args(["-c", "core.autocrlf=false", "apply", "--whitespace=error"])
            .arg(patch),
        "apply web input compatibility patch",
    )?;
    fs::write(source.join(MARKER), RECIPE)?;
    verify_tree(&source)?;
    fs::rename(&source, &dest)
        .context("install prepared platform (another preparer may already have finished)")
}

fn verify_tree(dest: &Path) -> Result<()> {
    let marker = fs::read_to_string(dest.join(MARKER))
        .context("Refusing to replace an unowned source directory")?;
    ensure!(
        marker.trim().len() == 64 && marker.trim().bytes().all(|c| c.is_ascii_hexdigit()),
        "Invalid platform source marker"
    );
    // Path + content digest for every file of the accepted public preparation.
    // The marker itself used to hash a script, which varies with checkout EOL.
    fn files(root: &Path, dir: &Path, names: &mut Vec<String>) -> Result<()> {
        for entry in dir.read_dir()? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let path = entry.path();
            if kind.is_dir() {
                files(root, &path, names)?;
            } else {
                ensure!(kind.is_file(), "Unexpected platform source entry");
                let name = path
                    .strip_prefix(root)?
                    .to_str()
                    .context("unexpected platform path")?
                    .replace('\\', "/");
                if name != MARKER {
                    names.push(name);
                }
            }
        }
        Ok(())
    }
    let mut names = vec![];
    files(dest, dest, &mut names)?;
    names.sort();
    let mut tree = Sha256::new();
    for name in names {
        tree.update(name.as_bytes());
        tree.update([0]);
        tree.update(Sha256::digest(fs::read(dest.join(name))?));
    }
    ensure!(
        format!("{:x}", tree.finalize())
            == "96e32370346b870cdeaf627fc3bbd1a55f915dbdf41dba3743f7acb8fd88037f",
        "Platform source differs; existing files left intact"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_to_replace_or_claim_an_existing_unowned_directory() {
        let d = tempfile::tempdir().unwrap();
        let dev = Dev::load(d.path()).unwrap();
        let dest = d.path().join("crates/web/.deps/gpui-pre-web");
        fs::create_dir_all(&dest).unwrap();
        let owned = dest.join("owner-edit");
        fs::write(&owned, "keep").unwrap();
        assert!(prepare(&dev).unwrap_err().to_string().contains("unowned"));
        assert_eq!(fs::read(owned).unwrap(), b"keep");
        assert!(!dest.join(MARKER).exists());
    }
    #[cfg(feature = "platform-smoke")]
    #[test]
    fn fresh_public_preparation_matches_original_tree_and_reuses_it() -> Result<()> {
        let d = tempfile::tempdir()?;
        let dev = Dev::load(d.path())?;
        prepare(&dev)?;
        prepare(&dev)?;
        let deps = d.path().join("crates/web/.deps");
        assert_eq!(deps.read_dir()?.count(), 1);
        let bad = deps.join("gpui-pre-web/extra.rs");
        fs::write(&bad, "outside the recipe")?;
        assert!(prepare(&dev).is_err());
        assert!(bad.is_file());
        Ok(())
    }
}
