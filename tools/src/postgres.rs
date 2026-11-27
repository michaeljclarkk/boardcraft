//! Local Windows SCRAM cluster, compatible with the existing current-user DPAPI files.
use crate::dev::Dev;
use anyhow::{Context, Result, bail, ensure};
use std::{
    env,
    ffi::OsStr,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use zeroize::Zeroizing;

const ADMIN: &str = "rustday_admin";
const APP: &str = "rustday_test";
const PORT: &str = "55437";
pub(crate) const URL_ENV: &str = "RUSTDAY_TEST_DATABASE_URL";

fn credential(root: &Path, role: &str) -> Result<Zeroizing<String>> {
    use windows_dpapi::{Scope, decrypt_data, encrypt_data};
    let path = root.join(format!("{role}.protected"));
    let file = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            ensure!(
                !root.join("data/PG_VERSION").try_exists()?,
                "Missing protected credential for {role}; inspect the existing cluster"
            );
            let mut random = Zeroizing::new([0u8; 32]);
            getrandom::fill(random.as_mut()).context("generate local password")?;
            let password = Zeroizing::new(hex::encode_upper(*random));
            let wide = Zeroizing::new(
                password
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>(),
            );
            let blob = encrypt_data(&wide, Scope::User, None)?;
            let mut tmp = tempfile::NamedTempFile::new_in(root)?;
            writeln!(tmp, "{}", hex::encode(blob))?;
            tmp.persist_noclobber(path)
                .context("save protected credential")?;
            return Ok(password);
        }
        Err(e) => return Err(e.into()),
    };
    // Set-Content in PowerShell may have emitted a BOM, depending on its edition.
    let encoded = if let Some(utf16) = file.strip_prefix(&[0xff, 0xfe]) {
        decode_wide(utf16)?
    } else {
        std::str::from_utf8(file.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&file))?.to_owned()
    };
    let blob = hex::decode(encoded.trim()).context("invalid protected credential encoding")?;
    ensure!(!blob.is_empty(), "Empty protected credential");
    let raw = Zeroizing::new(
        decrypt_data(&blob, Scope::User, None)
            .context("unlock current-user database credential")?,
    );
    let password = Zeroizing::new(decode_wide(&raw)?);
    ensure!(
        password.len() == 64 && password.bytes().all(|b| b.is_ascii_hexdigit()),
        "Unexpected generated credential format for {role}"
    );
    Ok(password)
}
fn decode_wide(bytes: &[u8]) -> Result<String> {
    ensure!(
        bytes.len().is_multiple_of(2),
        "Invalid protected credential length"
    );
    let units = Zeroizing::new(
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(u16::from_le_bytes)
            .collect::<Vec<_>>(),
    );
    Ok(String::from_utf16(&units)?)
}

fn root() -> Result<PathBuf> {
    Ok(PathBuf::from(
        env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is required for the local cluster")?,
    )
    .join("Rustday/postgres-dev"))
}
fn pg_command(dev: &Dev, bin: &str) -> Result<Command> {
    let mut cmd = dev.command(bin)?;
    // In particular, inherited PGHOSTADDR or PGSERVICE must not redirect a password.
    for (key, _) in env::vars_os() {
        if key.to_string_lossy().to_ascii_uppercase().starts_with("PG") {
            cmd.env_remove(key);
        }
    }
    Ok(cmd)
}
fn success(cmd: &mut Command, what: &str) -> Result<()> {
    ensure!(
        cmd.status().with_context(|| what.to_owned())?.success(),
        "{what} failed"
    );
    Ok(())
}

fn input(cmd: &mut Command, body: &[u8], what: &str) -> Result<()> {
    // psql can quote rejected SQL in errors. Do not forward output from secret-bearing input.
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| what.to_owned())?;
    let result = child
        .stdin
        .take()
        .context("missing child stdin")?
        .write_all(body);
    let status = child.wait()?;
    ensure!(status.success(), "{what} failed");
    result.context("write child stdin")
}

fn psql(dev: &Dev, port: &str, role: &str, database: &str, password: &str) -> Result<Command> {
    let mut cmd = pg_command(dev, "psql")?;
    cmd.env("PGPASSWORD", password).args([
        "-X",
        "-w",
        "-qAt",
        "-v",
        "ON_ERROR_STOP=1",
        "-h",
        "127.0.0.1",
        "-p",
        port,
        "-U",
        role,
        "-d",
        database,
    ]);
    Ok(cmd)
}

pub(crate) fn start(dev: &Dev) -> Result<Zeroizing<String>> {
    start_local(dev, &root()?, PORT)
}

fn start_local(dev: &Dev, root: &Path, port: &str) -> Result<Zeroizing<String>> {
    fs::create_dir_all(root)?;
    let data = root.join("data");
    let admin = credential(root, ADMIN)?;
    let app = credential(root, APP)?;
    if !data.join("PG_VERSION").try_exists()? {
        // initdb 17 requires a file. Keep it in the current user's private
        // cluster directory, exclusively created and unlinked on every exit.
        let mut pw = tempfile::NamedTempFile::new_in(root)?;
        pw.write_all(admin.as_bytes())?;
        pw.flush()?;
        let mut cmd = pg_command(dev, "initdb")?;
        cmd.arg("-D")
            .arg(&data)
            .args(["-U", ADMIN, "--pwfile"])
            .arg(pw.path())
            .args(["-A", "scram-sha-256", "-E", "UTF8", "--locale=C"]);
        let result = success(&mut cmd, "cluster initialization");
        pw.close()
            .context("remove temporary initdb password file")?;
        result?;
    }
    let ctl = || -> Result<Command> {
        let mut cmd = pg_command(dev, "pg_ctl")?;
        cmd.arg("-D").arg(&data);
        Ok(cmd)
    };
    match ctl()?.arg("status").stdout(Stdio::null()).status()?.code() {
        Some(0) => (),
        Some(3) => success(
            ctl()?
                .arg("-l")
                .arg(root.join("server.log"))
                .arg("-o")
                .arg(format!("-h 127.0.0.1 -p {port}"))
                .args(["-w", "start"]),
            "start cluster; check server.log and the port",
        )?,
        _ => bail!("Could not determine local cluster status"),
    }
    let admin_psql = || psql(dev, port, ADMIN, "postgres", &admin);
    let server = admin_psql()?.args(["-c", "SHOW data_directory"]).output()?;
    ensure!(
        server.status.success(),
        "Could not verify database instance"
    );
    let actual = Path::new(std::str::from_utf8(&server.stdout)?.trim()).canonicalize()?;
    ensure!(
        actual == data.canonicalize()?,
        "Unexpected database instance"
    );
    let sql = Zeroizing::new(format!(
        "SELECT 'CREATE ROLE rustday_test LOGIN PASSWORD ''{}''' WHERE NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'rustday_test') \\gexec\nSELECT 'CREATE DATABASE rustday_test OWNER rustday_test' WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'rustday_test') \\gexec\n",
        *app
    ));
    input(
        &mut admin_psql()?,
        sql.as_bytes(),
        "test database initialization",
    )?;
    let login = psql(dev, port, APP, APP, &app)?
        .args([
            "-c",
            "SELECT current_database(), current_user, current_setting('is_superuser')",
        ])
        .output()?;
    ensure!(
        login.status.success()
            && std::str::from_utf8(&login.stdout)?.trim() == "rustday_test|rustday_test|off",
        "Test login did not resolve to the unprivileged test role and database"
    );
    eprintln!("Connected to rustday_test as rustday_test");
    Ok(Zeroizing::new(format!(
        "postgresql://rustday_test:{}@127.0.0.1:{port}/rustday_test",
        *app
    )))
}

pub(crate) fn stop(dev: &Dev) -> Result<()> {
    stop_local(dev, &root()?)
}
fn stop_local(dev: &Dev, root: &Path) -> Result<()> {
    success(
        pg_command(dev, "pg_ctl")?
            .arg("-D")
            .arg(root.join("data"))
            .args(["-w", "stop"]),
        "stop local cluster",
    )
}

pub(crate) fn scope(cmd: &mut Command, url: &str) {
    cmd.env_remove("PGPASSWORD").env(URL_ENV, OsStr::new(url));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protected_files_survive_reopen_and_refuse_missing_or_corrupt_secrets() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let original = credential(root, ADMIN).unwrap();
        let path = root.join("rustday_admin.protected");
        let saved = fs::read_to_string(&path).unwrap();
        assert!(!saved.contains(&*original));
        let bom: Vec<u8> = [0xff, 0xfe]
            .into_iter()
            .chain(saved.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        fs::write(&path, bom).unwrap();
        assert!(credential(root, ADMIN).unwrap().as_bytes() == original.as_bytes());
        fs::write(&path, "00").unwrap();
        assert!(credential(root, ADMIN).is_err());
        fs::create_dir(root.join("data")).unwrap();
        fs::write(root.join("data/PG_VERSION"), "17").unwrap();
        assert!(credential(root, APP).is_err());
        assert!(!root.join("rustday_test.protected").exists());
    }

    #[cfg(feature = "postgres-smoke")]
    #[test]
    fn initialize_reuse_authenticate_and_stop_only_the_disposable_cluster() -> Result<()> {
        let d = tempfile::tempdir()?;
        let root = d.path().join("cluster with spaces");
        let dev = Dev::load(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())?;
        let port = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?
            .local_addr()?
            .port()
            .to_string();
        struct Cleanup<'a>(&'a Dev, &'a Path);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                if self.1.join("data/postmaster.pid").exists() {
                    let _ = stop_local(self.0, self.1);
                }
            }
        }
        let _cleanup = Cleanup(&dev, &root);
        let first = start_local(&dev, &root, &port)?;
        let second = start_local(&dev, &root, &port)?;
        ensure!(
            first.as_bytes() == second.as_bytes(),
            "reopening changed the test login"
        );
        let mut client = dev.command("cargo")?;
        scope(&mut client, &first);
        assert!(
            client
                .get_envs()
                .any(|(key, v)| key == URL_ENV && v == Some(OsStr::new(first.as_str())))
        );
        assert!(!client.get_args().any(|arg| arg == first.as_str()));
        assert!(
            dev.command("cargo")?
                .get_envs()
                .any(|(key, v)| key == URL_ENV && v.is_none())
        );
        stop_local(&dev, &root)?;
        assert!(!root.join("data/postmaster.pid").exists());
        assert!(root.join("data/PG_VERSION").exists());
        Ok(())
    }
}
