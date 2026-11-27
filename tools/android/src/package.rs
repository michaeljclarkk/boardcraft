use crate::{
    prepare,
    process::{self, SHORT, checked, text},
};
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

pub(crate) fn build(root: &Path, output: &Path) -> Result<()> {
    ensure!(
        output.is_absolute() && !output.exists(),
        "output-apk must be a new absolute file path"
    );
    let parent = output.parent().context("APK parent directory")?;
    ensure!(parent.is_dir(), "APK output parent must already exist");
    let _lease = process::build_lock(root)?;
    prepare::prepare(&root.join("android"))?;
    let mut command = process::cargo(
        root,
        &[
            "build",
            "--manifest-path",
            "android/Cargo.toml",
            "--locked",
            "--target",
            "x86_64-linux-android",
            "--lib",
            "--message-format=json-render-diagnostics",
        ],
    );
    // Compiler diagnostics are separate from the JSON artifact stream.
    let output_json = text(&mut command, Duration::from_secs(3600))?;
    let (library, proxy) = artifacts(&output_json)?;
    let sdk = sdk()?;
    let platform = sdk.join("platforms/android-35/android.jar");
    let tools = sdk.join("build-tools/35.0.0");
    let work = tempfile::Builder::new()
        .prefix("boardcraft-apk-")
        .tempdir_in(parent)?;
    let payload = work.path().join("payload");
    let lib = payload.join("lib/x86_64");
    fs::create_dir_all(&lib)?;
    fs::copy(library, lib.join("libboardcraft_kit_fixture.so"))?;
    let classes = work.path().join("classes");
    fs::create_dir(&classes)?;
    // The transparent IME proxy remains byte-for-byte upstream. No authored Java is packaged.
    checked(
        Command::new(process::tool("JAVA_HOME", "bin/javac.exe", "javac"))
            .args(["--release", "8", "-classpath"])
            .arg(&platform)
            .args(["-encoding", "UTF-8", "-d"])
            .arg(&classes)
            .arg(proxy),
        SHORT,
    )?;
    let mut bytecode = Vec::new();
    class_files(&classes, &mut bytecode)?;
    ensure!(!bytecode.is_empty(), "no upstream IME bytecode produced");
    checked(
        Command::new(tools.join("d8.bat"))
            .args(["--min-api", "28", "--lib"])
            .arg(&platform)
            .arg("--output")
            .arg(&payload)
            .args(bytecode),
        SHORT,
    )?;
    let unsigned = work.path().join("unsigned.apk");
    let aligned = work.path().join("aligned.apk");
    let signed = work.path().join("signed.apk");
    let key = work.path().join("fixture.p12");
    checked(
        Command::new(tools.join("aapt.exe"))
            .args(["package", "-M"])
            .arg(root.join("android/AndroidManifest.xml"))
            .arg("-I")
            .arg(platform)
            .arg("-F")
            .arg(&unsigned)
            .args(["-0", "so"])
            .arg(payload),
        SHORT,
    )?;
    checked(
        Command::new(tools.join("zipalign.exe"))
            .args(["-P", "16", "4"])
            .arg(unsigned)
            .arg(&aligned),
        SHORT,
    )?;
    // A new, disposable fixture key is created per invocation; no configured secret is read.
    checked(
        Command::new(process::tool("JAVA_HOME", "bin/keytool.exe", "keytool"))
            .arg("-genkeypair")
            .arg("-keystore")
            .arg(&key)
            .args([
                "-storetype",
                "PKCS12",
                "-storepass",
                "android",
                "-keypass",
                "android",
                "-alias",
                "fixture",
                "-keyalg",
                "RSA",
                "-keysize",
                "2048",
                "-validity",
                "30",
                "-dname",
                "CN=Boardcraft Debug Fixture",
            ]),
        SHORT,
    )?;
    checked(
        Command::new(tools.join("apksigner.bat"))
            .args(["sign", "--ks"])
            .arg(&key)
            .args([
                "--ks-pass",
                "pass:android",
                "--ks-key-alias",
                "fixture",
                "--out",
            ])
            .arg(&signed)
            .arg(aligned),
        SHORT,
    )?;
    checked(
        Command::new(tools.join("apksigner.bat"))
            .arg("verify")
            .arg(&signed),
        SHORT,
    )?;
    // persist_noclobber preserves an existing file even if it appeared during compilation.
    let mut published = tempfile::NamedTempFile::new_in(parent)?;
    std::io::copy(&mut fs::File::open(signed)?, published.as_file_mut())?;
    published.as_file().sync_all()?;
    published.persist_noclobber(output).map_err(|e| e.error)?;
    work.close()?;
    println!("Built signed x86_64 debug fixture");
    Ok(())
}

fn class_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            class_files(&entry.path(), files)?;
        } else if entry.path().extension().is_some_and(|e| e == "class") {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(())
}

pub(crate) fn sdk() -> Result<PathBuf> {
    std::env::var_os("ANDROID_HOME")
        .map(PathBuf::from)
        .context("set ANDROID_HOME to the fixture SDK")
}

fn artifacts(lines: &str) -> Result<(PathBuf, PathBuf)> {
    let mut library = None;
    let mut proxy = None;
    for line in lines.lines() {
        let record: serde_json::Value =
            serde_json::from_str(line).context("invalid Cargo artifact output")?;
        if record["reason"] != "compiler-artifact" {
            continue;
        }
        if record["target"]["name"] == "gpui_mobile" {
            let manifest = record["manifest_path"]
                .as_str()
                .context("mobile artifact manifest")?;
            proxy = Some(Path::new(manifest).parent().context("mobile manifest parent")?.join("example/android/gradle/app/src/main/java/dev/gpui/mobile/GpuiInputActivity.java"));
        } else if record["target"]["name"] == "boardcraft_kit_fixture" {
            library = record["filenames"].as_array().and_then(|files| {
                files
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(PathBuf::from)
                    .find(|p| {
                        p.file_name()
                            .is_some_and(|n| n == "libboardcraft_kit_fixture.so")
                    })
            });
        }
    }
    Ok((
        library.context("missing current fixture library artifact")?,
        proxy.context("missing current mobile host artifact")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_or_existing_apk_before_preparing_or_compiling() {
        let work = tempfile::tempdir().unwrap();
        let existing = work.path().join("fixture.apk");
        fs::write(&existing, b"preserve this output").unwrap();
        for output in [Path::new("relative.apk"), existing.as_path()] {
            assert_eq!(
                build(work.path(), output).unwrap_err().to_string(),
                "output-apk must be a new absolute file path"
            );
        }
        assert_eq!(fs::read(existing).unwrap(), b"preserve this output");
        assert!(!work.path().join("android/.deps").exists());
    }
}
