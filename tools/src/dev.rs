//! Apply the optional checkout tool paths to the requested child only.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    env,
    ffi::{OsStr, OsString},
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    tool_home: Option<PathBuf>,
    postgres_bin: Option<PathBuf>,
}

pub(crate) struct Dev {
    pub root: PathBuf,
    settings: Settings,
    path: OsString,
}

impl Dev {
    pub fn cargo_home(&self) -> Option<PathBuf> {
        self.settings
            .tool_home
            .as_ref()
            .map(|p| p.join("cargo"))
            .or_else(|| env::var_os("CARGO_HOME").map(Into::into))
            .or_else(|| env::home_dir().map(|p| p.join(".cargo")))
    }
    pub fn load(root: &Path) -> Result<Self> {
        let settings: Settings = match fs::read(root.join(".private/dev.json")) {
            Ok(bytes) => {
                serde_json::from_slice(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes))
                    .context("invalid .private/dev.json")?
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Settings::default(),
            Err(e) => return Err(e).context("read .private/dev.json"),
        };
        let path = env::join_paths(
            settings
                .postgres_bin
                .iter()
                .cloned()
                .chain(settings.tool_home.iter().map(|p| p.join("cargo/bin")))
                .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
        )?;
        Ok(Self {
            root: root.to_owned(),
            settings,
            path,
        })
    }

    pub fn command(&self, bin: impl AsRef<OsStr>) -> Result<Command> {
        let bin = bin.as_ref();
        // std::process routes .cmd/.bat through cmd.exe on Windows; keep argv literal.
        let ext = Path::new(bin)
            .extension()
            .and_then(OsStr::to_str)
            .unwrap_or("");
        ensure!(
            !cfg!(windows) || !(ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat")),
            "Expected a native executable, not a batch file"
        );
        let mut cmd = Command::new(bin);
        cmd.current_dir(&self.root)
            .env("PATH", &self.path)
            .env_remove("PGPASSWORD")
            .env_remove("RUSTDAY_TEST_DATABASE_URL");
        if let Some(home) = &self.settings.tool_home {
            cmd.env("RUSTUP_HOME", home.join("rustup"))
                .env("CARGO_HOME", home.join("cargo"));
            if env::var_os("CARGO_TARGET_DIR").is_none() {
                cmd.env("CARGO_TARGET_DIR", home.join("target"));
            }
        }
        Ok(cmd)
    }
}
